// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `job_run_events` table.

use sqlx::{SqliteConnection, SqlitePool};

use crate::{identifier::Id, storage::WriteOperation, time::Timestamp};

/// How loud one event is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventLevel {
    /// Detail nobody reads unless they are debugging the engine.
    Trace,
    /// Detail an operator reads when something is wrong.
    Debug,
    /// Normal operation.
    Info,
    /// Something that did not stop the run and should be looked at.
    Warn,
    /// Something that failed.
    Error,
}

impl EventLevel {
    /// The text stored in `job_run_events.level`.
    #[must_use]
    pub const fn as_text(self) -> &'static str {
        match self {
            Self::Trace => "Trace",
            Self::Debug => "Debug",
            Self::Info => "Info",
            Self::Warn => "Warn",
            Self::Error => "Error",
        }
    }
}

/// One line of a run's log, as the logs page reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEvent {
    /// The event's identifier.
    pub id: String,
    /// The run it belongs to.
    pub run_id: String,
    /// When it happened.
    pub at: Timestamp,
    /// How loud it is.
    pub level: String,
    /// What it is about — a definition, a library, a source, a wizard step.
    pub scope: Option<String>,
    /// The line itself.
    pub message: String,
}

/// Every event of one run, oldest first.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn events_for(readers: &SqlitePool, run_id: &str) -> Result<Vec<RunEvent>, sqlx::Error> {
    Ok(sqlx::query_as!(
        Row,
        "SELECT id, run_id, at, level, scope, message
         FROM job_run_events WHERE run_id = ?1 ORDER BY at, id",
        run_id
    )
    .fetch_all(readers)
    .await?
    .into_iter()
    .map(RunEvent::from)
    .collect())
}

/// Appends one line to a run's log.
#[derive(Debug)]
pub struct AppendRunEvent {
    /// The identifier to assign.
    pub id: Id,
    /// The run this belongs to.
    pub run_id: String,
    /// How loud it is.
    pub level: EventLevel,
    /// What it is about.
    pub scope: Option<String>,
    /// The line itself.
    pub message: String,
    /// When it happened.
    pub at: Timestamp,
}

impl WriteOperation for AppendRunEvent {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let id = self.id.as_str().to_owned();
        let level = self.level.as_text();
        let at = self.at.as_millis();
        sqlx::query!(
            "INSERT INTO job_run_events (id, run_id, at, level, scope, message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            id,
            self.run_id,
            at,
            level,
            self.scope,
            self.message
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

/// The `job_run_events` row exactly as `SQLite` returns it.
struct Row {
    id: String,
    run_id: String,
    at: i64,
    level: String,
    scope: Option<String>,
    message: String,
}

impl From<Row> for RunEvent {
    fn from(row: Row) -> Self {
        Self {
            id: row.id,
            run_id: row.run_id,
            at: Timestamp::from_millis(row.at),
            level: row.level,
            scope: row.scope,
            message: row.message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_level_renders_a_value_the_schema_allows() {
        let allowed = ["Trace", "Debug", "Info", "Warn", "Error"];
        for level in [
            EventLevel::Trace,
            EventLevel::Debug,
            EventLevel::Info,
            EventLevel::Warn,
            EventLevel::Error,
        ] {
            assert!(allowed.contains(&level.as_text()), "{level:?}");
        }
    }
}
