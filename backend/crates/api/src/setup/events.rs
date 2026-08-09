// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Recording what the wizard did, where the logs page already looks.

use afisharr_core::{
    identifier::Id,
    jobs::{AppendRunEvent, EventLevel, RunTrigger, StartRun, find_open_run},
};

use crate::state::ApiState;

/// The `job_id` the wizard's single run is filed under.
///
/// A stable name rather than a ULID, and no `jobs` row: the schema's
/// `job_runs.job_id` deliberately carries no foreign key because runs outlive
/// the jobs that made them, and the wizard never had one.
pub const SETUP_JOB_ID: &str = "setup";

/// Appends one line for a completed wizard step.
///
/// Best effort by design. A step that succeeded and could not be logged has
/// still succeeded, and failing the operator's setup over an audit line would
/// be the tail wagging the dog. The failure is reported through `tracing`,
/// which is where an unwritable database belongs.
pub async fn record_step(state: &ApiState, step: &str, message: &str) {
    let now = state.clock().now();
    let Some(run_id) = open_run(state).await else {
        return;
    };

    let appended = state
        .database()
        .writer()
        .submit(AppendRunEvent {
            id: Id::generate(state.clock()),
            run_id,
            level: EventLevel::Info,
            scope: Some(step.to_owned()),
            message: message.to_owned(),
            at: now,
        })
        .await;

    if let Err(error) = appended {
        tracing::warn!(%error, step, "could not record a setup step");
    }
}

/// The one open run for this instance's setup, opening it if it is the first
/// step.
///
/// One run for the whole wizard, not one per step: PRD §19.6.1 puts every step
/// under a single `Api`-triggered row so the logs page shows setup as one
/// sequence rather than as eight unrelated runs.
async fn open_run(state: &ApiState) -> Option<String> {
    let existing = find_open_run(state.database().readers(), SETUP_JOB_ID)
        .await
        .inspect_err(|error| tracing::warn!(%error, "could not read the setup run"))
        .ok()
        .flatten();
    if let Some(run_id) = existing {
        return Some(run_id);
    }

    state
        .database()
        .writer()
        .submit(StartRun {
            id: Id::generate(state.clock()),
            job_id: SETUP_JOB_ID.to_owned(),
            trigger: RunTrigger::Api,
            actor: None,
            at: state.clock().now(),
        })
        .await
        .inspect_err(|error| tracing::warn!(%error, "could not open the setup run"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wizard_files_its_run_under_a_stable_name() {
        // A ULID here would make every restart a new job, and the logs page
        // filters by job.
        assert_eq!(SETUP_JOB_ID, "setup");
    }
}
