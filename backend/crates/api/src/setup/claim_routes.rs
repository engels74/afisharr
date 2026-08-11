// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `POST /api/setup/claim`.
//!
//! The first door onto the claim: the token the container printed to its
//! console, which is what proves console access (PRD §19.6.1). The second is
//! `recover_routes`, and what either one opens is `claim_lease`.

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use afisharr_core::setup::{CLAIM_COOKIE, ClaimState, inspect};
use axum::{Json, extract::State};
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    error::{AppError, AppResult, ErrorCode, JsonBody, Problem},
    proxy::ClientContext,
    setup::{
        claim_lease::{ClaimGranted, grant, held_elsewhere, mint_cookie_value, spend_attempt},
        events::record_step,
    },
    state::ApiState,
};

/// The token an operator copies off the console.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimRequest {
    /// The three four-character segments, as printed.
    pub token: String,
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
    //    consume the token (PRD §19.6.1). Normalised first: see
    //    [`normalize_token`].
    if !state
        .bootstrap()
        .accepts(&normalize_token(&request.token), now)
    {
        return Err(token_refused());
    }

    let granted = grant(&state, client, jar, mint_cookie_value(), now).await;
    if granted.is_ok() {
        record_step(&state, "claim", "The setup wizard was claimed.").await;
    }
    granted
}

/// The token as the operator meant it, before it is compared.
///
/// Surrounding whitespace and letter case are transport damage, not a wrong
/// token. The value is copied off a container console, and a terminal that
/// takes the trailing newline with it — or an operator who types the printed
/// lower-case alphabet with a capital — produced a length mismatch or a byte
/// mismatch, one refusal that blames the token, and one spent attempt out of
/// the five this address gets every fifteen minutes. Five of those and the only
/// door into a brand-new instance is shut for a quarter of an hour, with the
/// message telling the operator to restart the container over a stray space.
///
/// It narrows nothing an attacker can use: the alphabet is `a`–`z0-9`, so
/// case folding maps no two distinct tokens onto one, and the guess space is
/// the 62 bits PRD §19.6.1 claims either way. The comparison after it is still
/// constant-time, and this runs on the caller's own copy of a value it already
/// holds.
fn normalize_token(token: &str) -> String {
    token.trim().to_ascii_lowercase()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pasted_token_survives_the_whitespace_the_terminal_added() {
        // The exact shapes a console copy produces. Each of these was a spent
        // attempt out of five, and five of them locked the operator out of a
        // brand-new instance for fifteen minutes.
        assert_eq!(normalize_token("abcd-efgh-ijkl\n"), "abcd-efgh-ijkl");
        assert_eq!(normalize_token("  abcd-efgh-ijkl  "), "abcd-efgh-ijkl");
        assert_eq!(normalize_token("ABCD-EFGH-IJKL"), "abcd-efgh-ijkl");
    }

    #[test]
    fn normalizing_merges_no_two_tokens_this_instance_can_mint() {
        // The alphabet is `a`-`z0-9`, so case folding is a no-op on anything
        // `BootstrapToken::mint` produces and the 62-bit guess space stands.
        for byte in afisharr_core::setup::TOKEN_SHAPE.alphabet {
            let character = String::from(char::from(*byte));
            assert_eq!(normalize_token(&character), character);
        }
    }

    #[test]
    fn every_bad_token_gets_one_indistinguishable_refusal() {
        let refusal = token_refused();
        assert_eq!(refusal.problem().code, ErrorCode::Unauthenticated);
        assert!(refusal.problem().pointer.is_none());
        assert!(refusal.problem().mismatch.is_none());
    }

    #[test]
    fn the_request_body_rejects_a_field_it_does_not_know() {
        assert!(
            serde_json::from_str::<ClaimRequest>(r#"{"token":"a","step":8}"#).is_err(),
            "a client must not be able to name a step"
        );
    }
}
