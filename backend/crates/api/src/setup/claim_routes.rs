// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `POST /api/setup/claim` and `POST /api/setup/recover`.

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use afisharr_core::{
    accounts::{self, User},
    entropy,
    setup::{CLAIM_COOKIE, CLAIM_TTL_MILLIS, ClaimOutcome, ClaimState, MintClaim, inspect},
    time::Timestamp,
};
use axum::{Json, extract::State};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    authentication::session::csrf_cookie,
    error::{AppError, AppResult, ErrorCode, JsonBody, Problem},
    proxy::ClientContext,
    ratelimit::{Bucket, Decision},
    security::{CSRF_COOKIE, set},
    setup::events::record_step,
    state::ApiState,
};

/// The token an operator copies off the console.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimRequest {
    /// The three four-character segments, as printed.
    pub token: String,
}

/// The administrator credentials that recover an interrupted setup.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoverRequest {
    /// The administrator's account name.
    pub username: String,
    /// The administrator's password.
    pub password: String,
}

/// A claim now held by this browser.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimGranted {
    /// When the hold lapses if nothing renews it, in epoch milliseconds.
    pub expires_at: i64,
}

/// Takes the wizard for this browser, on the strength of the console token.
#[utoipa::path(
    post,
    path = "/api/setup/claim",
    tag = "setup",
    request_body = ClaimRequest,
    responses(
        (status = 200, description = "The wizard is now held by this browser", body = ClaimGranted),
        (status = 400, description = "The request body was not readable", body = Problem),
        (status = 401, description = "The token was not accepted", body = Problem),
        (status = 409, description = "Another browser holds the wizard", body = Problem),
        (status = 429, description = "Too many attempts", body = Problem),
    ),
)]
pub async fn claim(
    State(state): State<ApiState>,
    client: ClientContext,
    jar: CookieJar,
    JsonBody(request): JsonBody<ClaimRequest>,
) -> AppResult<(CookieJar, Json<ClaimGranted>)> {
    let now = state.clock().now();
    let existing = jar
        .get(CLAIM_COOKIE)
        .map(|cookie| cookie.value().to_owned());

    // 1. The holder renews and succeeds, before anything else is consulted.
    if let ClaimState::HeldByCaller { .. } =
        inspect(state.database().readers(), existing.as_deref(), now)
            .await
            .map_err(AppError::internal)?
    {
        return grant(&state, client, jar, existing.unwrap_or_default(), now).await;
    }

    // 2. Held elsewhere: answer before the limiter is touched, so refreshing
    //    the page costs nothing an operator will need later (PRD §21.4.3).
    if let ClaimState::HeldByAnother { expires_at } =
        inspect(state.database().readers(), existing.as_deref(), now)
            .await
            .map_err(AppError::internal)?
    {
        return Err(held_elsewhere(now, expires_at));
    }

    // 3. Now the limiter, guarding the one step where guessing gains anything.
    spend_attempt(&state, client)?;

    // 4. And only then the comparison, which is constant-time and does not
    //    consume the token (PRD §19.6.1).
    if !state.bootstrap().accepts(&request.token, now) {
        return Err(token_refused());
    }

    let granted = grant(&state, client, jar, mint_cookie_value(), now).await;
    if granted.is_ok() {
        record_step(&state, "claim", "The setup wizard was claimed.").await;
    }
    granted
}

/// Takes the wizard on administrator credentials, once an account exists.
///
/// The recovery path (PRD §19.6.1): the token dies with the process, the
/// account does not, so an interrupted setup survives a restart.
#[utoipa::path(
    post,
    path = "/api/setup/recover",
    tag = "setup",
    request_body = RecoverRequest,
    responses(
        (status = 200, description = "The wizard is now held by this browser", body = ClaimGranted),
        (status = 400, description = "The request body was not readable", body = Problem),
        (status = 401, description = "The credentials were not accepted", body = Problem),
        (status = 409, description = "Another browser holds the wizard", body = Problem),
        (status = 429, description = "Too many attempts", body = Problem),
    ),
)]
pub async fn recover(
    State(state): State<ApiState>,
    client: ClientContext,
    jar: CookieJar,
    JsonBody(request): JsonBody<RecoverRequest>,
) -> AppResult<(CookieJar, Json<ClaimGranted>)> {
    let now = state.clock().now();

    if let ClaimState::HeldByAnother { expires_at } = inspect(state.database().readers(), None, now)
        .await
        .map_err(AppError::internal)?
    {
        return Err(held_elsewhere(now, expires_at));
    }

    // Two limits, because this route verifies an administrator's password and
    // so is the sign-in route wearing another name. `SetupAttempt` is counted
    // per address and carries no lockout, which is all a claim token needs —
    // the token dies with the process. A password does not: an attacker who
    // can vary their address gets the whole per-address allowance again from
    // every address they can reach the instance from, and rotating addresses
    // is a residential proxy away. `LoginAccount` is the bucket that does not
    // move with them, and its escalating lockout is why `log_in` spends it for
    // the identical check (PRD §21.4.3).
    let account_bucket = Bucket::login_account(&request.username);
    refuse_if_limited(&state, &account_bucket, client)?;
    spend_attempt(&state, client)?;

    let account = accounts::find_by_username(state.database().readers(), &request.username)
        .await
        .map_err(AppError::internal)?
        .filter(|user| user.is_admin && user.is_active());

    if !verify_admin(account.as_ref(), request.password).await? {
        // Recorded on failure only, for the reason `log_in` records it that
        // way: a limit that counted successes would lock an operator out of
        // their own interrupted setup for signing in twice.
        let _ = state
            .limiter()
            .record(&account_bucket, Some(client.address));
        // The same refusal for an unknown username and a wrong password: a
        // different one tells a guesser which of the two they achieved.
        return Err(credentials_refused());
    }
    state
        .limiter()
        .forget(&account_bucket, Some(client.address));

    let granted = grant(&state, client, jar, mint_cookie_value(), now).await;
    if granted.is_ok() {
        record_step(
            &state,
            "claim",
            "The setup wizard was recovered with administrator credentials.",
        )
        .await;
    }
    granted
}

/// Mints or renews the claim and attaches the cookies.
///
/// Two cookies, not one, and for the same reason signing in sets two: the claim
/// is an ambient credential a browser attaches to any request another origin
/// can cause, so the CSRF check applies to it, and the check needs a token the
/// page can echo (PRD §21.4.2).
async fn grant(
    state: &ApiState,
    client: ClientContext,
    jar: CookieJar,
    cookie_value: String,
    now: Timestamp,
) -> AppResult<(CookieJar, Json<ClaimGranted>)> {
    let outcome = state
        .database()
        .writer()
        .submit(MintClaim {
            cookie_value: cookie_value.clone(),
            at: now,
        })
        .await
        .map_err(AppError::internal)?;

    let expires_at = match outcome {
        ClaimOutcome::Granted { expires_at } => expires_at,
        ClaimOutcome::Blocked { expires_at } => return Err(held_elsewhere(now, expires_at)),
    };

    // Only when the browser does not already hold one: a renewal that rotated
    // the token would invalidate the value a form read a moment ago and refuse
    // the submission that follows.
    let mut jar = jar;
    if jar.get(CSRF_COOKIE).is_none() {
        jar = jar.add(csrf_cookie(client.scheme));
    }
    let jar = jar.add(set(
        CLAIM_COOKIE,
        cookie_value,
        "/api/setup",
        CLAIM_TTL_MILLIS / 1000,
        client.scheme,
        true,
    ));
    Ok((
        jar,
        Json(ClaimGranted {
            expires_at: expires_at.as_millis(),
        }),
    ))
}

/// Verifies an administrator's password, at constant cost either way.
async fn verify_admin(account: Option<&User>, password: String) -> AppResult<bool> {
    let Some(stored) = account.and_then(|user| user.password_hash.clone()) else {
        // No account, or a Plex account with no password: still spend the time,
        // so "no administrator by that name" and "wrong password" are
        // indistinguishable from the outside.
        return Ok(accounts::verify(password, absent_hash())
            .await
            .unwrap_or(false));
    };
    accounts::verify(password, stored)
        .await
        .map_err(AppError::internal)
}

fn absent_hash() -> String {
    crate::authentication::ABSENT_ACCOUNT_HASH.to_owned()
}

fn spend_attempt(state: &ApiState, client: ClientContext) -> AppResult<()> {
    match state
        .limiter()
        .record(&Bucket::SetupAttempt, Some(client.address))
    {
        Decision::Allowed => Ok(()),
        Decision::Refused {
            retry_after_seconds,
        } => Err(AppError::new(
            Problem::new(
                ErrorCode::RateLimited,
                "Too many setup attempts from this address. Try again later.",
            )
            .retry_after(retry_after_seconds),
        )),
    }
}

/// Refuses when `bucket` is already spent, without spending it again.
fn refuse_if_limited(state: &ApiState, bucket: &Bucket, client: ClientContext) -> AppResult<()> {
    match state.limiter().check(bucket, Some(client.address)) {
        Decision::Allowed => Ok(()),
        Decision::Refused {
            retry_after_seconds,
        } => Err(AppError::new(
            Problem::new(
                ErrorCode::RateLimited,
                "Too many attempts against that account. Try again later.",
            )
            .retry_after(retry_after_seconds),
        )),
    }
}

/// A fresh, unguessable cookie value for a new claim.
fn mint_cookie_value() -> String {
    let bytes = entropy::bytes::<32>();
    let mut out = String::with_capacity(64);
    for byte in bytes {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

fn held_elsewhere(now: Timestamp, expires_at: Timestamp) -> AppError {
    let seconds = u64::try_from(now.millis_until(expires_at).max(1000) / 1000).unwrap_or(1);
    AppError::new(
        Problem::new(
            ErrorCode::Blocked,
            "Another browser is holding the setup wizard.",
        )
        .retry_after(seconds),
    )
}

/// One refusal for wrong, expired, malformed, and empty.
///
/// Distinguishing them tells a guesser which of the four they achieved
/// (PRD §7.14), and none of the four is rendered differently by the interface.
fn token_refused() -> AppError {
    AppError::of(
        ErrorCode::Unauthenticated,
        "That setup token was not accepted. Restart the container and read the console \
         for a fresh one.",
    )
}

fn credentials_refused() -> AppError {
    AppError::of(
        ErrorCode::Unauthenticated,
        "Those administrator credentials were not accepted.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claim_cookie_value_is_sixty_four_hex_characters() {
        let value = mint_cookie_value();
        assert_eq!(value.len(), 64);
        assert!(value.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(value, mint_cookie_value());
    }

    #[test]
    fn every_bad_token_gets_one_indistinguishable_refusal() {
        let refusal = token_refused();
        assert_eq!(refusal.problem().code, ErrorCode::Unauthenticated);
        assert!(refusal.problem().pointer.is_none());
        assert!(refusal.problem().mismatch.is_none());
    }

    #[test]
    fn a_held_claim_reports_when_it_lapses_and_nothing_about_who_holds_it() {
        let error = held_elsewhere(Timestamp::EPOCH, Timestamp::from_millis(600_000));
        assert_eq!(error.problem().code, ErrorCode::Blocked);
        assert_eq!(error.problem().retry_after_seconds, Some(600));
    }

    #[test]
    fn the_request_bodies_reject_fields_they_do_not_know() {
        assert!(
            serde_json::from_str::<ClaimRequest>(r#"{"token":"a","step":8}"#).is_err(),
            "a client must not be able to name a step"
        );
        assert!(
            serde_json::from_str::<RecoverRequest>(r#"{"username":"a","password":"b","admin":1}"#)
                .is_err()
        );
    }
}
