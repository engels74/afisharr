// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Job runs and the events they append.
//!
//! The scheduler, its cron parsing, and the jobs themselves arrive later. What
//! exists here is the run record and its event log, because the setup wizard
//! already needs somewhere to say what it did: PRD §19.6.1 puts each wizard
//! step in `job_run_events` under one `Api`-triggered run, so the logs page
//! reads them with the filters it already has and no second surface is
//! invented for them.
//!
//! They are deliberately **not** the lifecycle audit record. §21.4.8 reserves
//! that for explaining what the engine did, not for forensics against the
//! operator, and a wizard step is an operator action.

mod events;
mod runs;

pub use events::{AppendRunEvent, EventLevel, RunEvent, events_for};
pub use runs::{
    FinishRun, RunStatus, RunTrigger, StartRun, find as find_run, find_open as find_open_run,
};
