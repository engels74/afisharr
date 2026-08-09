// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `job_runs` table.

use sqlx::{SqliteConnection, SqlitePool};

use crate::{identifier::Id, storage::WriteOperation, time::Timestamp};

/// What caused a run to start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunTrigger {
    /// A schedule came due.
    Schedule,
    /// An operator pressed something.
    Manual,
    /// An API call, including the setup wizard's steps.
    Api,
    /// Startup recovery.
    Startup,
    /// Another run required it.
    Dependency,
}

impl RunTrigger {
    /// The text stored in `job_runs.trigger`.
    #[must_use]
    pub const fn as_text(self) -> &'static str {
        match self {
            Self::Schedule => "Schedule",
            Self::Manual => "Manual",
            Self::Api => "Api",
            Self::Startup => "Startup",
            Self::Dependency => "Dependency",
        }
    }
}

/// How a run ended, or that it has not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    /// Still going.
    Running,
    /// Finished, everything worked.
    Ok,
    /// Finished, nothing worked.
    Failed,
    /// Stopped on request.
    Cancelled,
    /// Did not need to run.
    Skipped,
    /// Finished, some of it worked.
    ///
    /// Its own status rather than a failure: four sources of five succeeding is
    /// four sources of data the interface must show (PRD §8.4).
    PartialSuccess,
}

impl RunStatus {
    /// The text stored in `job_runs.status`.
    #[must_use]
    pub const fn as_text(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Ok => "Ok",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
            Self::Skipped => "Skipped",
            Self::PartialSuccess => "PartialSuccess",
        }
    }
}

/// Reads one run's status, if it exists.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn find(readers: &SqlitePool, run_id: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar!("SELECT status FROM job_runs WHERE id = ?1", run_id)
        .fetch_optional(readers)
        .await
}

/// The newest still-running run of `job_id`, if there is one.
///
/// The setup wizard uses it to keep every step under one run rather than
/// opening a new one per step (PRD §19.6.1).
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn find_open(readers: &SqlitePool, job_id: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT id FROM job_runs WHERE job_id = ?1 AND status = 'Running'
         ORDER BY started_at DESC LIMIT 1",
        job_id
    )
    .fetch_optional(readers)
    .await
}

/// Opens a run.
///
/// `job_id` is a free string with no foreign key, exactly as the schema has it:
/// runs outlive the jobs that produced them, and the setup wizard's run has no
/// `jobs` row at all.
#[derive(Debug)]
pub struct StartRun {
    /// The identifier to assign.
    pub id: Id,
    /// The job this run belongs to, or a stable name for a run with no job.
    pub job_id: String,
    /// What caused it.
    pub trigger: RunTrigger,
    /// Who caused it, when a person did.
    pub actor: Option<String>,
    /// When it started.
    pub at: Timestamp,
}

impl WriteOperation for StartRun {
    type Output = String;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<String, sqlx::Error> {
        let id = self.id.as_str().to_owned();
        let trigger = self.trigger.as_text();
        let status = RunStatus::Running.as_text();
        let at = self.at.as_millis();
        sqlx::query!(
            "INSERT INTO job_runs (id, job_id, trigger, actor, started_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            id,
            self.job_id,
            trigger,
            self.actor,
            at,
            status
        )
        .execute(&mut *conn)
        .await?;
        Ok(id)
    }
}

/// Closes a run.
#[derive(Debug)]
pub struct FinishRun {
    /// The run being closed.
    pub run_id: String,
    /// How it ended.
    pub status: RunStatus,
    /// A one-line failure, when there is one.
    pub error: Option<String>,
    /// When it ended.
    pub at: Timestamp,
}

impl WriteOperation for FinishRun {
    type Output = bool;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
        let status = self.status.as_text();
        let at = self.at.as_millis();
        let affected = sqlx::query!(
            "UPDATE job_runs SET status = ?2, finished_at = ?3, error = ?4
             WHERE id = ?1 AND status = 'Running'",
            self.run_id,
            status,
            at,
            self.error
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();
        Ok(affected == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_trigger_renders_a_value_the_schema_allows() {
        let allowed = ["Schedule", "Manual", "Api", "Startup", "Dependency"];
        for trigger in [
            RunTrigger::Schedule,
            RunTrigger::Manual,
            RunTrigger::Api,
            RunTrigger::Startup,
            RunTrigger::Dependency,
        ] {
            assert!(allowed.contains(&trigger.as_text()), "{trigger:?}");
        }
    }

    #[test]
    fn every_status_renders_a_value_the_schema_allows() {
        let allowed = [
            "Running",
            "Ok",
            "Failed",
            "Cancelled",
            "Skipped",
            "PartialSuccess",
        ];
        for status in [
            RunStatus::Running,
            RunStatus::Ok,
            RunStatus::Failed,
            RunStatus::Cancelled,
            RunStatus::Skipped,
            RunStatus::PartialSuccess,
        ] {
            assert!(allowed.contains(&status.as_text()), "{status:?}");
        }
    }
}
