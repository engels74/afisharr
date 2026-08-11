// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The lease itself: taking it, renewing it, and refusing to.
//!
//! Two doors open onto one lease — the console token (`claim_routes`) and the
//! administrator's credentials (`recover_routes`) — and what happens once
//! either is accepted is the same thing. It lives here so the two cannot drift:
//! a second `grant` that minted the cookie a little differently, or a second
//! refusal that named a different retry, would be two accounts of one lease
//! and only one of them would be the one anybody tested (P7).

use afisharr_core::{
    entropy,
    setup::{CLAIM_COOKIE, CLAIM_TTL_MILLIS, ClaimOutcome, MintClaim},
    time::Timestamp,
};
use axum::Json;
use axum_extra::extract::CookieJar;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    authentication::session::csrf_cookie,
    error::{AppError, AppResult, ErrorCode, Problem},
    proxy::ClientContext,
    ratelimit::Bucket,
    security::{CSRF_COOKIE, set},
    state::ApiState,
};

/// A claim now held by this browser.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimGranted {
    /// When the hold lapses if nothing renews it, in epoch milliseconds.
    pub expires_at: i64,
}

/// Mints or renews the claim and attaches the cookies.
///
/// Two cookies, not one, and for the same reason signing in sets two: the claim
/// is an ambient credential a browser attaches to any request another origin
/// can cause, so the CSRF check applies to it, and the check needs a token the
/// page can echo (PRD §21.4.2).
pub(super) async fn grant(
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

pub(super) fn spend_attempt(state: &ApiState, client: ClientContext) -> AppResult<()> {
    state.limiter().spend(
        &Bucket::SetupAttempt,
        Some(client.address),
        "Too many setup attempts from this address. Try again later.",
    )
}

/// Refuses when `bucket` is already spent, without spending it again.
pub(super) fn refuse_if_limited(
    state: &ApiState,
    bucket: &Bucket,
    client: ClientContext,
) -> AppResult<()> {
    state.limiter().refuse_if_spent(
        bucket,
        Some(client.address),
        "Too many attempts against that account. Try again later.",
    )
}

/// A fresh, unguessable cookie value for a new claim.
pub(super) fn mint_cookie_value() -> String {
    let bytes = entropy::bytes::<32>();
    let mut out = String::with_capacity(64);
    for byte in bytes {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

pub(super) fn held_elsewhere(now: Timestamp, expires_at: Timestamp) -> AppError {
    let seconds = u64::try_from(now.millis_until(expires_at).max(1000) / 1000).unwrap_or(1);
    AppError::new(
        Problem::new(
            ErrorCode::Blocked,
            "Another browser is holding the setup wizard.",
        )
        .retry_after(seconds),
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
    fn a_held_claim_reports_when_it_lapses_and_nothing_about_who_holds_it() {
        let error = held_elsewhere(Timestamp::EPOCH, Timestamp::from_millis(600_000));
        assert_eq!(error.problem().code, ErrorCode::Blocked);
        assert_eq!(error.problem().retry_after_seconds, Some(600));
    }
}
