// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `GET /api/setup/status` and `POST /api/setup/complete`.

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use afisharr_core::{
    accounts,
    jobs::RunStatus,
    setup::{CLAIM_COOKIE, ClaimState, CompleteSetup, Evidence, SetupStep, inspect, read_evidence},
};
use axum::{Json, extract::State};
use axum_extra::extract::CookieJar;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    error::{AppError, AppResult, ErrorCode, Problem},
    proxy::ClientContext,
    security::expire,
    setup::{
        StepView,
        events::{finish_run, record_step},
    },
    state::ApiState,
};

/// Where the wizard is, derived from what the database holds.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    /// The step to resume at.
    pub step: StepView,
    /// Its position in the journey, one-based, for display only.
    pub ordinal: u8,
    /// Whether this browser holds the claim.
    pub claim_held: bool,
    /// Whether an administrator account exists, which decides whether the
    /// claim page offers the recovery affordance (PRD §7.14).
    pub recovery_available: bool,
    /// Whether a token is live to be entered at all.
    ///
    /// False after fifteen minutes with no restart, which is what turns the
    /// claim page's message from "enter the token" into "restart the container
    /// and read the console".
    pub token_live: bool,
}

/// Reports the step this instance resumes at.
///
/// The step is computed here and never accepted from the caller. A step index
/// in a query string, a cookie, or a client-held draft would let a caller name
/// the step they would like to be on, which on the claim step means naming
/// step 2 (D-046, `I-UX-10`). There is no parameter on this route to supply
/// one with.
#[utoipa::path(
    get,
    path = "/api/setup/status",
    tag = "setup",
    responses(
        (status = 200, description = "The derived step", body = SetupStatus),
        (status = 403, description = "The claim on this wizard has expired", body = Problem),
        (status = 404, description = "Setup has already been completed", body = Problem),
        (status = 429, description = "Too many requests", body = Problem),
    ),
)]
pub async fn status(State(state): State<ApiState>, jar: CookieJar) -> AppResult<Json<SetupStatus>> {
    let now = state.clock().now();
    let cookie_value = jar
        .get(CLAIM_COOKIE)
        .map(|cookie| cookie.value().to_owned());

    let claim = inspect(state.database().readers(), cookie_value.as_deref(), now)
        .await
        .map_err(AppError::internal)?;
    let evidence: Evidence = read_evidence(state.database().readers(), claim)
        .await
        .map_err(AppError::internal)?;

    let step = SetupStep::resume_at(evidence);
    Ok(Json(SetupStatus {
        step: StepView::from(step),
        ordinal: step.ordinal(),
        claim_held: matches!(claim, ClaimState::HeldByCaller { .. }),
        recovery_available: evidence.admin_exists
            && !matches!(claim, ClaimState::HeldByAnother { .. }),
        token_live: state.bootstrap().is_live(now),
    }))
}

/// Finishes setup.
///
/// Four things happen together and none is optional (PRD §19.6.1):
/// `instance.setup_completed_at` is written, the `setup:claim` lease is
/// deleted, the in-memory token is cleared, and the cookie is expired. From
/// then on the banner prints nothing on restart and these endpoints answer 404.
///
/// All four are irreversible, which is why the prerequisite is checked first.
#[utoipa::path(
    post,
    path = "/api/setup/complete",
    tag = "setup",
    responses(
        (status = 200, description = "Setup is finished", body = SetupStatus),
        (status = 403, description = "The claim on this wizard has expired", body = Problem),
        (status = 404, description = "Setup has already been completed", body = Problem),
        (status = 409, description = "Another browser holds the wizard, or no administrator exists yet", body = Problem),
        (status = 429, description = "Too many requests", body = Problem),
    ),
)]
pub async fn complete(
    State(state): State<ApiState>,
    client: ClientContext,
    jar: CookieJar,
) -> AppResult<(CookieJar, Json<SetupStatus>)> {
    let now = state.clock().now();

    // Nothing below is reversible. Completion writes `setup_completed_at`,
    // deletes the claim, and clears the token, and from that moment the setup
    // routes answer 404 while `require_setup_completed` refuses everything
    // else. On an instance with no administrator that is a permanent lockout:
    // no credential exists to sign in with, the recovery door needs the account
    // that does not exist, and the door that would create one is gone. A caller
    // holding only a valid claim must not be able to reach it (`I-SEC-8`).
    if !accounts::admin_exists(state.database().readers())
        .await
        .map_err(AppError::internal)?
    {
        return Err(AppError::of(
            ErrorCode::Conflict,
            "Setup cannot be completed until an administrator account exists.",
        ));
    }

    state
        .database()
        .writer()
        .submit(CompleteSetup { at: now })
        .await
        .map_err(AppError::internal)?;

    state.bootstrap().clear();
    state.mark_setup_completed();
    record_step(&state, "review", "Setup was completed.").await;
    finish_run(&state, RunStatus::Ok).await;

    let jar = jar.add(expire(CLAIM_COOKIE, "/api/setup", client.scheme));
    Ok((
        jar,
        Json(SetupStatus {
            step: StepView::Review,
            ordinal: SetupStep::Review.ordinal(),
            claim_held: false,
            recovery_available: false,
            token_live: false,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_body_carries_the_step_as_a_name_and_an_ordinal() {
        let encoded = serde_json::to_value(SetupStatus {
            step: StepView::Integrations,
            ordinal: SetupStep::Integrations.ordinal(),
            claim_held: true,
            recovery_available: false,
            token_live: true,
        })
        .expect("serialises");
        assert_eq!(encoded["step"], "integrations");
        assert_eq!(encoded["ordinal"], 5);
        assert_eq!(encoded["claimHeld"], true);
    }

    #[test]
    fn completion_reports_the_final_step_and_no_live_token() {
        let encoded = serde_json::to_value(SetupStatus {
            step: StepView::Review,
            ordinal: SetupStep::Review.ordinal(),
            claim_held: false,
            recovery_available: false,
            token_live: false,
        })
        .expect("serialises");
        assert_eq!(encoded["step"], "review");
        assert_eq!(encoded["tokenLive"], false);
    }
}
