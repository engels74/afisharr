// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! First run: the console proof, the browser's claim on the wizard, and the
//! step the wizard resumes at.
//!
//! D-029 assumes the instance may be reachable from the internet, which makes
//! "the first visitor becomes the administrator" a race the operator loses to
//! a port scanner. Two mechanisms close it (PRD §19.6.1): a bootstrap token
//! printed to the console proves console access, and a `setup:claim` lease
//! converts that one-time proof into an exclusive, time-boxed hold on the
//! wizard, bound to one browser.
//!
//! The step is derived here and never carried by the client (D-046). A step
//! index in a query string lets a caller name the step they would like to be
//! on, which on the claim step means naming step 2.

mod acknowledgements;
mod claim;
mod evidence;
mod steps;
mod token;

pub use acknowledgements::{AckSetupStep, CompleteSetup, PACKS_ACK, REPORT_ACK};
pub use claim::{
    CLAIM_COOKIE, CLAIM_TTL_MILLIS, ClaimOutcome, ClaimState, MintClaim, RenewClaim, inspect,
};
pub use evidence::{Evidence, read as read_evidence};
pub use steps::SetupStep;
pub use token::{BootstrapToken, TOKEN_LIFETIME_MILLIS, TOKEN_SHAPE, TokenStore};
