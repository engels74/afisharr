// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! First run: the claim, the gate over every wizard endpoint, and the step.
//!
//! Three doors, and the order they are tried in is the security of the whole
//! thing (PRD §19.6.1):
//!
//! 1. The holder of the current claim renews it and succeeds.
//! 2. A caller who is not the holder is told when the hold lapses — **before**
//!    the limiter is consulted, so an operator refreshing the page does not
//!    spend the attempts they will need once it does.
//! 3. Only then does the limiter guard the token comparison, which is the one
//!    step where guessing gains anything.
//!
//! Once an administrator exists, that account's credentials are a second way
//! in, so an interrupted setup survives the restart that destroys the token.

pub(crate) mod admin;
mod claim_lease;
pub(crate) mod claim_routes;
pub(crate) mod claim_status;
mod events;
mod gate;
pub(crate) mod recover_routes;
pub(crate) mod status;
mod step_view;

pub use admin::{CreateAdmin, create_admin};
pub use claim_lease::ClaimGranted;
pub use claim_routes::{ClaimRequest, claim};
pub use claim_status::{ClaimStatus, claim_status};
pub use events::{SETUP_JOB_ID, record_step};
pub use gate::{require_claim, require_setup_incomplete};
pub use recover_routes::{RecoverRequest, recover};
pub use status::{SetupStatus, complete, status};
pub use step_view::StepView;
