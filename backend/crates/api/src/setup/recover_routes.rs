// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `POST /api/setup/recover`.
//!
//! The second of the two doors onto the claim. The first is `claim_routes`,
//! and what either one opens is `claim_lease`.

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use afisharr_core::{
    accounts,
    setup::{CLAIM_COOKIE, ClaimState, inspect},
};
use axum::{Json, extract::State};
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    authentication::verify_password,
    error::{AppError, AppResult, ErrorCode, JsonBody, Problem},
    proxy::ClientContext,
    ratelimit::Bucket,
    setup::{
        claim_lease::{
            ClaimGranted, grant, held_elsewhere, mint_cookie_value, spend_attempt, take_attempt,
        },
        events::record_step,
    },
    state::ApiState,
};

/// The administrator credentials that recover an interrupted setup.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoverRequest {
    /// The administrator's account name.
    pub username: String,
    /// The administrator's password.
    pub password: String,
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
        // As `claim` declares it, and reachable for the same reason: a browser
        // recovering a claim it still holds carries the claim cookie, which is
        // an ambient credential, so the cross-site layer judges this request
        // and refuses it when the echoed token is absent or stale (§24.5).
        (status = 403, description = "The request was refused as cross-site", body = Problem),
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
    // The caller's own cookie, exactly as `claim` reads it. Passing `None` here
    // would classify a claim *this* browser holds as held by another and refuse
    // it — which is the one case recovery exists for: the container restarted,
    // so the console token died with the process, while the lease row and this
    // browser's cookie both survived. Both doors would then be shut, and the
    // operator would wait out a claim that is already theirs.
    let existing = jar
        .get(CLAIM_COOKIE)
        .map(|cookie| cookie.value().to_owned());

    let held = inspect(state.database().readers(), existing.as_deref(), now)
        .await
        .map_err(AppError::internal)?;
    if let ClaimState::HeldByAnother { expires_at } = held {
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
    //
    // Both are taken here, before the verification, for the reason `log_in`
    // gives: a counter read before the hash and written after it lets a burst
    // through whole, because none of the attempts in it has failed yet.
    let account_bucket = Bucket::login_account(&request.username);
    take_attempt(&state, &account_bucket, client)?;
    spend_attempt(&state, client)?;

    let account = accounts::find_by_username(state.database().readers(), &request.username)
        .await
        .map_err(AppError::internal)?
        .filter(|user| user.is_admin && user.is_active());

    if !verify_password(account.as_ref(), request.password).await? {
        // Nothing is counted here: the attempt was taken above. The same
        // refusal for an unknown username and a wrong password, because a
        // different one tells a guesser which of the two they achieved.
        return Err(credentials_refused());
    }
    // Handed back on success only, so an operator recovering their own
    // interrupted setup is not locked out for doing it twice.
    state
        .limiter()
        .forget(&account_bucket, Some(client.address));

    // The holder's own value when they already hold the lease, exactly as
    // `claim` renews. A fresh value would hash to a different lease owner, so
    // `MintClaim` would neither renew nor take and would report the operator's
    // own claim as blocking them.
    let cookie_value = match held {
        ClaimState::HeldByCaller { .. } => existing.unwrap_or_else(mint_cookie_value),
        ClaimState::Unclaimed | ClaimState::HeldByAnother { .. } => mint_cookie_value(),
    };

    let granted = grant(&state, client, jar, cookie_value, now).await;
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
    fn an_unknown_name_and_a_wrong_password_get_the_same_refusal() {
        // Distinguishing them tells a guesser which of the two they achieved.
        let refusal = credentials_refused();
        assert_eq!(refusal.problem().code, ErrorCode::Unauthenticated);
        assert!(refusal.problem().pointer.is_none());
    }

    #[test]
    fn the_request_body_rejects_a_field_it_does_not_know() {
        assert!(
            serde_json::from_str::<RecoverRequest>(r#"{"username":"a","password":"b","admin":1}"#)
                .is_err()
        );
    }
}
