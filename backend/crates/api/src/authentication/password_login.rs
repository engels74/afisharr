// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Local sign-in, sign-out, and "who am I".

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use afisharr_core::{
    accounts::{self, TouchLastLogin, User},
    time::Timestamp,
};
use axum::{Json, extract::State, http::header::USER_AGENT};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    authentication::{Authenticated, session},
    error::{AppError, AppResult, ErrorCode, JsonBody, Problem},
    proxy::ClientContext,
    ratelimit::{Bucket, Decision},
    state::ApiState,
};

/// A PHC string no password verifies against.
///
/// Verified against when the username is unknown, so an unknown account costs
/// the same quarter-second as a wrong password. Without it, sign-in is a
/// username oracle that answers in single-digit milliseconds.
pub(crate) const ABSENT_ACCOUNT_HASH: &str = "$argon2id$v=19$m=65536,t=2,p=1$\
    b+hBjerprIEAZe5xVF9rvQ$eOa9cVGup14UK8k8/VOkO5D8I/fsVNg/ejjps/+PC8E";

/// What the sign-in form sends.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Credentials {
    /// The account name.
    pub username: String,
    /// The password.
    pub password: String,
}

/// Who the caller now is.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SignedIn {
    /// The account's identifier.
    pub user_id: String,
    /// The account name.
    pub username: String,
    /// The name to show, when it differs from the account name.
    pub display_name: Option<String>,
    /// Whether the account holds administrator rights.
    pub is_admin: bool,
}

impl From<&User> for SignedIn {
    fn from(user: &User) -> Self {
        Self {
            user_id: user.id.clone(),
            username: user.username.clone(),
            display_name: user.display_name.clone(),
            is_admin: user.is_admin,
        }
    }
}

/// Signs in with a username and a password.
///
/// The limiter is consulted before the hash and recorded after it, and only on
/// failure: a limit that counted successes would lock out the operator signing
/// in from a fourth device (PRD §21.4.3).
#[utoipa::path(
    post,
    path = "/api/auth/login",
    tag = "authentication",
    request_body = Credentials,
    responses(
        (status = 200, description = "Signed in", body = SignedIn),
        (status = 400, description = "The request body was not readable", body = Problem),
        (status = 401, description = "The credentials were not accepted", body = Problem),
        (status = 429, description = "Too many attempts", body = Problem),
    ),
)]
pub async fn log_in(
    State(state): State<ApiState>,
    client: ClientContext,
    jar: CookieJar,
    headers: axum::http::HeaderMap,
    JsonBody(credentials): JsonBody<Credentials>,
) -> AppResult<(CookieJar, Json<SignedIn>)> {
    // Keyed by the account name alone: the limiter drops the address for this
    // bucket, so five failures is five failures however many source addresses
    // they arrive from. Counting it per address would hand an attacker who can
    // rotate their address the whole allowance again, every time (PRD §21.4.3).
    //
    // Built through the constructor rather than the variant, because the name
    // is the caller's to choose and the constructor is where its length stops
    // being the caller's to choose.
    let account_bucket = Bucket::login_account(&credentials.username);
    refuse_if_limited(&state, &account_bucket, client)?;
    refuse_if_limited(&state, &Bucket::LoginAddress, client)?;

    let now = state.clock().now();
    let user = accounts::find_by_username(state.database().readers(), &credentials.username)
        .await
        .map_err(AppError::internal)?;

    let accepted = verify(user.as_ref(), credentials.password).await?;
    if !accepted {
        record_failure(&state, &account_bucket, client);
        record_failure(&state, &Bucket::LoginAddress, client);
        return Err(rejected());
    }

    let Some(user) = user.filter(User::is_active) else {
        record_failure(&state, &account_bucket, client);
        record_failure(&state, &Bucket::LoginAddress, client);
        return Err(rejected());
    };

    state
        .limiter()
        .forget(&account_bucket, Some(client.address));
    state
        .limiter()
        .forget(&Bucket::LoginAddress, Some(client.address));

    sign_in(&state, &user, client, &headers, jar, now).await
}

/// Signs out, revoking the session that made the request.
#[utoipa::path(
    post,
    path = "/api/auth/logout",
    tag = "authentication",
    responses(
        (status = 204, description = "Signed out"),
        (status = 401, description = "No session to sign out of", body = Problem),
    ),
)]
pub async fn log_out(
    State(state): State<ApiState>,
    client: ClientContext,
    caller: Authenticated,
    jar: CookieJar,
) -> AppResult<(CookieJar, axum::http::StatusCode)> {
    let mut jar = jar;
    if let Some(digest) = caller.session_digest() {
        for cookie in session::revoke(&state, digest, client.scheme).await? {
            jar = jar.add(cookie);
        }
    }
    Ok((jar, axum::http::StatusCode::NO_CONTENT))
}

/// Reports the signed-in account.
#[utoipa::path(
    get,
    path = "/api/auth/session",
    tag = "authentication",
    responses(
        (status = 200, description = "The signed-in account", body = SignedIn),
        (status = 401, description = "Nobody is signed in", body = Problem),
    ),
)]
pub async fn whoami(
    State(state): State<ApiState>,
    caller: Authenticated,
) -> AppResult<Json<SignedIn>> {
    let user = accounts::find_by_id(state.database().readers(), &caller.user_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(rejected)?;
    Ok(Json(SignedIn::from(&user)))
}

/// Mints the session and attaches its cookies.
async fn sign_in(
    state: &ApiState,
    user: &User,
    client: ClientContext,
    headers: &axum::http::HeaderMap,
    jar: CookieJar,
    now: Timestamp,
) -> AppResult<(CookieJar, Json<SignedIn>)> {
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    let issued = session::issue(state, &user.id, client, user_agent).await?;
    let _ = state
        .database()
        .writer()
        .submit(TouchLastLogin {
            user_id: user.id.clone(),
            at: now,
        })
        .await;

    let mut jar = jar;
    for cookie in issued.cookies {
        jar = jar.add(cookie);
    }
    Ok((jar, Json(SignedIn::from(user))))
}

/// Verifies a password against an account that may not exist.
async fn verify(user: Option<&User>, password: String) -> AppResult<bool> {
    let stored = user
        .and_then(|user| user.password_hash.clone())
        .unwrap_or_else(|| ABSENT_ACCOUNT_HASH.to_owned());
    match accounts::verify(password, stored).await {
        Ok(accepted) => Ok(accepted),
        // A stored hash that will not parse is a corrupt row, not a wrong
        // password. Reporting it as a wrong password would send the operator
        // to reset a credential that is fine.
        Err(error) => Err(AppError::internal(error)),
    }
}

fn refuse_if_limited(state: &ApiState, bucket: &Bucket, client: ClientContext) -> AppResult<()> {
    match state.limiter().check(bucket, Some(client.address)) {
        Decision::Allowed => Ok(()),
        Decision::Refused {
            retry_after_seconds,
        } => Err(AppError::new(
            Problem::new(
                ErrorCode::RateLimited,
                "Too many sign-in attempts. Try again later.",
            )
            .retry_after(retry_after_seconds),
        )),
    }
}

fn record_failure(state: &ApiState, bucket: &Bucket, client: ClientContext) {
    let _ = state.limiter().record(bucket, Some(client.address));
}

/// One refusal for a wrong password, an unknown account, and a disabled one.
fn rejected() -> AppError {
    AppError::of(
        ErrorCode::Unauthenticated,
        "That username and password were not accepted.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_unknown_account_verifies_against_the_absent_hash_and_fails() {
        assert!(
            !verify(None, "anything".to_owned())
                .await
                .expect("the absent hash must parse")
        );
    }

    #[tokio::test]
    async fn the_absent_account_hash_is_a_readable_phc_string() {
        // If this string ever stops parsing, every unknown-username sign-in
        // becomes a 500 and the timing oracle it exists to close reopens.
        assert!(
            afisharr_core::accounts::verify("x".to_owned(), ABSENT_ACCOUNT_HASH.to_owned())
                .await
                .is_ok()
        );
    }

    #[test]
    fn a_signed_in_body_carries_no_credential_material() {
        let user = User {
            id: "U".to_owned(),
            kind: afisharr_core::accounts::UserKind::Local,
            username: "operator".to_owned(),
            email: None,
            display_name: Some("Operator".to_owned()),
            password_hash: Some("$argon2id$secret".to_owned()),
            plex_account_id: None,
            plex_uuid: None,
            avatar_url: None,
            is_admin: true,
            created_at: Timestamp::EPOCH,
            updated_at: Timestamp::EPOCH,
            last_login_at: None,
            disabled_at: None,
        };
        let encoded = serde_json::to_string(&SignedIn::from(&user)).expect("serialises");
        assert!(!encoded.contains("argon2"), "{encoded}");
        assert!(encoded.contains("\"isAdmin\":true"), "{encoded}");
    }

    #[test]
    fn the_credentials_body_rejects_a_field_it_does_not_know() {
        let error = serde_json::from_str::<Credentials>(
            r#"{"username":"a","password":"b","isAdmin":true}"#,
        )
        .expect_err("an unknown field must be refused");
        assert!(error.to_string().contains("isAdmin"), "{error}");
    }
}
