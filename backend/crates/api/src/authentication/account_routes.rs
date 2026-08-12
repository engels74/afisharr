// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Changing a password, and managing the sessions that outlive it.

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use afisharr_core::{
    accounts::{self, PasswordRotation, RotatePassword},
    sessions::{self, Session, SessionToken},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header::USER_AGENT},
};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    authentication::{AccountManage, Authenticated, Scoped, SessionsManage, session},
    error::{AppError, AppResult, ErrorCode, JsonBody, Problem},
    proxy::ClientContext,
    state::ApiState,
};

/// The shortest password this instance will store.
///
/// The same floor the first-run account is held to; stated once and used from
/// both places rather than drifting apart (P7).
pub(crate) const MINIMUM_PASSWORD_LENGTH: usize = 12;

/// What a password change sends.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PasswordChange {
    /// The password in force now, re-entered.
    pub current_password: String,
    /// The password to store instead.
    pub new_password: String,
}

/// What a password change produced.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasswordChanged {
    /// How many other sessions were revoked.
    ///
    /// Reported rather than silent: the point of revoking them is that the
    /// operator knows the other devices are signed out.
    pub sessions_revoked: u64,
}

/// One session, as the settings page lists it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    /// The digest, which is what revocation names.
    ///
    /// Safe to show: it is the SHA-256 of a cookie value, and knowing it does
    /// not let anyone present the value it came from.
    pub id: String,
    /// Whether this is the session making the request.
    pub is_current: bool,
    /// The user agent recorded when it was created.
    pub user_agent: Option<String>,
    /// The address it was created from.
    pub ip: Option<String>,
    /// When it was created, in epoch milliseconds.
    pub created_at: i64,
    /// When it was last used, in epoch milliseconds.
    pub last_seen_at: i64,
    /// When it was revoked, in epoch milliseconds.
    pub revoked_at: Option<i64>,
}

/// Changes the signed-in account's password.
///
/// Everything PRD §21.4.2 asks for happens here and happens together, in one
/// transaction: the current password is re-verified, the new hash is written,
/// every session for the account is revoked, and a replacement is inserted for
/// the browser that asked. Rotation is not decoration — a session identifier
/// that survives a password change survives the theft the change was made to
/// end — and a rotation split across separate commits has a window in which
/// exactly that is true.
#[utoipa::path(
    post,
    path = "/api/settings/password",
    tag = "settings",
    request_body = PasswordChange,
    responses(
        (status = 200, description = "The password is changed", body = PasswordChanged),
        (status = 400, description = "The new password was refused", body = Problem),
        (status = 401, description = "The current password was not accepted", body = Problem),
        (status = 403, description = "That key was not issued with the scope this route needs, or setup has not been completed on this instance", body = Problem),
        (status = 409, description = "That account has no password to change, or another change reached it first", body = Problem),
        (status = 429, description = "Too many requests", body = Problem),
    ),
)]
pub async fn change_password(
    State(state): State<ApiState>,
    client: ClientContext,
    Scoped(caller, _): Scoped<AccountManage>,
    jar: CookieJar,
    headers: axum::http::HeaderMap,
    JsonBody(request): JsonBody<PasswordChange>,
) -> AppResult<(CookieJar, Json<PasswordChanged>)> {
    if request.new_password.chars().count() < MINIMUM_PASSWORD_LENGTH {
        return Err(AppError::new(
            Problem::new(ErrorCode::Invalid, "That password is too short.")
                .at("/newPassword")
                .expecting(
                    format!("at least {MINIMUM_PASSWORD_LENGTH} characters"),
                    format!("{} characters", request.new_password.chars().count()),
                ),
        ));
    }

    let user = accounts::find_by_id(state.database().readers(), &caller.user_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::of(ErrorCode::Unauthenticated, "Sign in to continue."))?;

    let Some(stored) = user.password_hash.clone() else {
        return Err(AppError::of(
            ErrorCode::Conflict,
            "This account signs in through Plex and has no password to change.",
        ));
    };
    if !accounts::verify(request.current_password, stored.clone())
        .await
        .map_err(AppError::internal)?
    {
        return Err(AppError::new(
            Problem::new(
                ErrorCode::Unauthenticated,
                "That current password was not accepted.",
            )
            .at("/currentPassword"),
        ));
    }

    let hashed = accounts::hash(request.new_password)
        .await
        .map_err(AppError::internal)?;

    // One transaction, and the cookies are attached only after it commits. The
    // three writes are the rotation guarantee: a password that changed while
    // the identifiers it protected stayed valid has ended nothing, including
    // the theft the operator performed it to end (PRD §21.4.2).
    let replacement = replacement_for(&caller);
    let rotated = state
        .database()
        .writer()
        .submit(RotatePassword {
            user_id: user.id.clone(),
            expected_hash: stored,
            password_hash: hashed,
            current_session: caller.session_digest().map(str::to_owned),
            replacement_digest: replacement.as_ref().map(|token| token.digest().to_owned()),
            user_agent: headers
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            ip: Some(client.address.to_string()),
            at: state.clock().now(),
        })
        .await
        .map_err(AppError::internal)?;

    let others_revoked = match rotated {
        PasswordRotation::Rotated { others_revoked } => others_revoked,
        // Another change verified the same password and committed first. This
        // one is rotating away a credential that is already gone: writing it
        // would revoke the replacement session that change's browser is
        // holding, and hand the account to whichever request finished last.
        PasswordRotation::Superseded => {
            return Err(AppError::of(
                ErrorCode::Conflict,
                "That password was already changed by another request. \
                 Sign in with the new password.",
            ));
        }
        // The account was read and its current password verified a moment ago,
        // so finding no local row to change means it was deleted or disabled in
        // between. Nothing was written.
        PasswordRotation::NoLocalAccount => {
            return Err(AppError::of(
                ErrorCode::Conflict,
                "That account can no longer be changed. Sign in again.",
            ));
        }
    };

    let mut jar = jar;
    if let Some(replacement) = replacement.as_ref() {
        for cookie in session::cookies_for(replacement, client.scheme) {
            jar = jar.add(cookie);
        }
    }
    Ok((
        jar,
        Json(PasswordChanged {
            // The caller's own rotated-away session is not one of these:
            // counting it would overstate what happened on other devices by
            // exactly one, and an API-key caller has none to count at all.
            sessions_revoked: others_revoked,
        }),
    ))
}

/// The session to issue in place of the one this change revokes.
///
/// `None` for an API-key caller, and that is the point. Rotation replaces the
/// credential the caller presented; a bearer key is not one of the credentials
/// this change touches, so there is nothing to replace. Minting a session
/// anyway wrote a 30-day administrator credential nobody asked for and returned
/// its plaintext cookie in the body of an API call — into the response log of
/// whatever script rotated the password, and into any proxy log along the way —
/// leaving an orphan row that the sessions page shows as somebody else's device
/// and that no operator has a reason to revoke (`I-SEC-8`).
fn replacement_for(caller: &Authenticated) -> Option<SessionToken> {
    caller.session_digest().map(|_| SessionToken::generate())
}

/// Lists the signed-in account's sessions.
#[utoipa::path(
    get,
    path = "/api/settings/sessions",
    tag = "settings",
    responses(
        (status = 200, description = "Every session, newest first", body = Vec<SessionView>),
        (status = 401, description = "No accepted credential was presented", body = Problem),
        (status = 403, description = "That key was not issued with the scope this route needs, or setup has not been completed on this instance", body = Problem),
        (status = 429, description = "Too many requests", body = Problem),
    ),
)]
pub async fn list_sessions(
    State(state): State<ApiState>,
    Scoped(caller, _): Scoped<SessionsManage>,
) -> AppResult<Json<Vec<SessionView>>> {
    let current = caller.session_digest().map(str::to_owned);
    let sessions = sessions::list_for_user(state.database().readers(), &caller.user_id)
        .await
        .map_err(AppError::internal)?;
    Ok(Json(
        sessions
            .into_iter()
            .map(|session| view(session, current.as_deref()))
            .collect(),
    ))
}

/// Revokes one of the signed-in account's sessions.
#[utoipa::path(
    delete,
    path = "/api/settings/sessions/{id}",
    tag = "settings",
    params(("id" = String, Path, description = "The session to revoke")),
    responses(
        (status = 204, description = "The session is revoked"),
        (status = 401, description = "No accepted credential was presented", body = Problem),
        (status = 403, description = "That key was not issued with the scope this route needs, or setup has not been completed on this instance", body = Problem),
        (status = 404, description = "No such session on this account", body = Problem),
        (status = 429, description = "Too many requests", body = Problem),
    ),
)]
pub async fn revoke_session(
    State(state): State<ApiState>,
    Scoped(caller, _): Scoped<SessionsManage>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    // Scoped to the caller's own sessions: an admin-only surface is still not
    // a surface where one account's identifier revokes another's.
    let owned = sessions::find_by_digest(state.database().readers(), &id)
        .await
        .map_err(AppError::internal)?
        .is_some_and(|session| session.user_id == caller.user_id);
    if !owned {
        return Err(AppError::of(
            ErrorCode::NotFound,
            "That session does not exist on this account.",
        ));
    }

    state
        .database()
        .writer()
        .submit(afisharr_core::sessions::RevokeSession {
            digest: id,
            at: state.clock().now(),
        })
        .await
        .map_err(AppError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}

fn view(session: Session, current: Option<&str>) -> SessionView {
    SessionView {
        is_current: current.is_some_and(|digest| digest == session.digest),
        id: session.digest,
        user_agent: session.user_agent,
        ip: session.ip,
        created_at: session.created_at.as_millis(),
        last_seen_at: session.last_seen_at.as_millis(),
        revoked_at: session
            .revoked_at
            .map(afisharr_core::time::Timestamp::as_millis),
    }
}

#[cfg(test)]
mod tests {
    use afisharr_core::time::Timestamp;

    use super::*;

    fn session(digest: &str) -> Session {
        Session {
            digest: digest.to_owned(),
            user_id: "U".to_owned(),
            created_at: Timestamp::from_millis(1),
            expires_at: Timestamp::from_millis(2),
            last_seen_at: Timestamp::from_millis(3),
            user_agent: Some("Firefox".to_owned()),
            ip: Some("10.0.0.1".to_owned()),
            revoked_at: None,
        }
    }

    #[test]
    fn the_session_making_the_request_is_marked_as_current() {
        assert!(view(session("a"), Some("a")).is_current);
        assert!(!view(session("b"), Some("a")).is_current);
    }

    #[test]
    fn an_api_key_caller_marks_no_session_as_current() {
        assert!(!view(session("a"), None).is_current);
    }

    fn caller(credential: crate::authentication::Credential) -> Authenticated {
        Authenticated {
            user_id: "U".to_owned(),
            is_admin: true,
            credential,
        }
    }

    #[test]
    fn a_browser_gets_the_session_that_replaces_the_one_it_lost() {
        let replacement = replacement_for(&caller(crate::authentication::Credential::Session {
            digest: "d".to_owned(),
        }));
        assert!(
            replacement.is_some(),
            "the browser that asked is signed out by the rotation and must be signed back in"
        );
    }

    #[test]
    fn an_api_key_caller_is_handed_no_session_at_all() {
        // The failure this closes: a script rotating the password over a bearer
        // key was answered with `Set-Cookie: afisharr_session=<plaintext>`, so
        // a 30-day administrator credential landed in its response log.
        assert!(
            replacement_for(&caller(crate::authentication::Credential::ApiKey {
                id: "k".to_owned(),
                scopes: afisharr_core::api_keys::ScopeSet::NONE,
            }))
            .is_none()
        );
    }

    #[test]
    fn the_password_change_body_rejects_a_field_it_does_not_know() {
        assert!(
            serde_json::from_str::<PasswordChange>(
                r#"{"currentPassword":"a","newPassword":"b","userId":"U"}"#
            )
            .is_err(),
            "a caller must not be able to name whose password to change"
        );
    }
}
