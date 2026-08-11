// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `GET /api/setup/claim` — what the claim page needs before it has a claim.

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use afisharr_core::{
    accounts,
    setup::{CLAIM_COOKIE, ClaimState, SetupStep, inspect},
};
use axum::{Json, extract::State};
use axum_extra::extract::CookieJar;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    error::{AppError, AppResult, Problem},
    state::ApiState,
};

/// The two facts the claim page renders, and whether it already holds a claim.
///
/// Deliberately not the derived step and deliberately not `Evidence`: this route
/// is outside the claim gate, so it says the least that lets step one draw
/// itself and nothing about how far setup has got (D-046).
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimStatus {
    /// The claim step's position in the journey, one-based, for display only.
    ///
    /// Carried rather than assumed, for the same reason `/api/setup/status`
    /// carries it: the shape of the journey is the server's to state, and a
    /// number written into a page is a number that goes stale in silence.
    pub ordinal: u8,
    /// Whether this browser already holds the wizard.
    pub claim_held: bool,
    /// Whether an administrator account exists, which decides whether the claim
    /// page offers the recovery affordance (PRD §7.14).
    pub recovery_available: bool,
    /// Whether a token is live to be entered at all.
    ///
    /// False fifteen minutes after a start with no restart, which is what turns
    /// the page's message from "enter the token" into "restart the container
    /// and read the console".
    pub token_live: bool,
}

/// Reports what the claim page needs, without a claim.
///
/// `/api/setup/status` sits behind the claim gate, so an unclaimed browser can
/// never read it — which is correct for the derived step and useless for the
/// page that has to be drawn *before* a claim exists. Left to guess, the
/// interface fabricates: a hard-coded `recoveryAvailable: false` hides the
/// recovery form from the operator whose token has died with a restart, and a
/// hard-coded `tokenLive: true` offers a token field on an instance that has
/// no live token to accept.
///
/// Neither fact is a disclosure. "An administrator exists" is already the
/// difference between `/api/setup/recover` accepting credentials and refusing
/// them, and "a token is live" is already the difference between a claim
/// attempt being refused and being possible at all.
#[utoipa::path(
    get,
    path = "/api/setup/claim",
    tag = "setup",
    responses(
        (status = 200, description = "What the claim page renders", body = ClaimStatus),
        (status = 404, description = "Setup has already been completed", body = Problem),
        (status = 429, description = "Too many requests", body = Problem),
    ),
)]
pub async fn claim_status(
    State(state): State<ApiState>,
    jar: CookieJar,
) -> AppResult<Json<ClaimStatus>> {
    let now = state.clock().now();
    let cookie_value = jar
        .get(CLAIM_COOKIE)
        .map(|cookie| cookie.value().to_owned());

    let claim = inspect(state.database().readers(), cookie_value.as_deref(), now)
        .await
        .map_err(AppError::internal)?;
    let admin_exists = accounts::admin_exists(state.database().readers())
        .await
        .map_err(AppError::internal)?;

    Ok(Json(ClaimStatus {
        ordinal: SetupStep::Claim.ordinal(),
        claim_held: matches!(claim, ClaimState::HeldByCaller { .. }),
        // Offered only when it can work: recovery mints a claim, and a claim
        // held by another browser is refused whoever asks (PRD §7.14).
        recovery_available: admin_exists && !matches!(claim, ClaimState::HeldByAnother { .. }),
        token_live: state.bootstrap().is_live(now),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_carries_the_claim_step_and_nothing_about_how_far_setup_has_got() {
        // The derived step is behind the gate. A step name here would let a
        // caller read how far setup has got without holding the wizard (D-046).
        let encoded = serde_json::to_value(ClaimStatus {
            ordinal: 1,
            claim_held: false,
            recovery_available: true,
            token_live: false,
        })
        .expect("serialises");
        let mut keys: Vec<&str> = encoded
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["claimHeld", "ordinal", "recoveryAvailable", "tokenLive"]
        );
        assert_eq!(encoded["ordinal"], 1);
    }
}
