// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The lifecycle system's persistent side.
//!
//! Every side effect the lifecycle takes is *intended*, then *executed*, then
//! *confirmed*, and startup re-drives every intent that never reached
//! `Confirmed` (PRD §17.9). Phase 0 owns the half of that contract which
//! belongs to startup: releasing the ownership a crashed process left on an
//! open intent, so the executor can pick it up. The executor itself is built
//! with the state machine.

mod intents;

pub use intents::{IntentState, ReleaseStaleIntents};
