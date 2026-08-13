// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Recording what the wizard did, where the logs page already looks.

use afisharr_core::{
    identifier::Id,
    jobs::{AppendRunEvent, EventLevel, FinishRun, RunStatus, RunTrigger, StartRun, find_open_run},
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

/// Closes the wizard's run, recording how it ended.
///
/// `record_step` finds or opens a run and never closes one. Without this, a
/// setup that finished perfectly leaves a `job_runs` row reading `Running`
/// forever — and `status` is the column the logs page filters on and the column
/// `find_open_run` matches, so the next wizard on the same instance would also
/// append into a run that ended months ago.
///
/// Best effort, for the same reason `record_step` is: setup has succeeded by
/// the time this is called, and failing it over an audit line would be the tail
/// wagging the dog.
pub async fn finish_run(state: &ApiState, status: RunStatus) {
    let Some(run_id) = find_open_run(state.database().readers(), SETUP_JOB_ID)
        .await
        .inspect_err(|error| tracing::warn!(%error, "could not read the setup run"))
        .ok()
        .flatten()
    else {
        return;
    };

    let finished = state
        .database()
        .writer()
        .submit(FinishRun {
            run_id,
            status,
            error: None,
            at: state.clock().now(),
        })
        .await;

    if let Err(error) = finished {
        tracing::warn!(%error, "could not close the setup run");
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
