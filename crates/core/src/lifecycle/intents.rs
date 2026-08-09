// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `lifecycle_intents` state column, and startup's claim on open intents.

use sqlx::SqliteConnection;

use crate::{storage::WriteOperation, time::Timestamp};

/// Where an intent has got to.
///
/// A closed set with a `CHECK` behind it, so it is an enum: an intent in an
/// unlisted state is a correctness bug that must never reach disk (PRD §19.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntentState {
    /// Recorded, not yet started.
    Intended,
    /// A process is executing it now.
    Executing,
    /// The side effect happened; the result has not been observed yet.
    Executed,
    /// The result was observed. Terminal.
    Confirmed,
    /// The attempt failed and may be retried.
    Failed,
    /// Given up on. Terminal.
    Abandoned,
}

impl IntentState {
    /// The exact token stored in `lifecycle_intents.state`.
    #[must_use]
    pub const fn as_token(self) -> &'static str {
        match self {
            Self::Intended => "Intended",
            Self::Executing => "Executing",
            Self::Executed => "Executed",
            Self::Confirmed => "Confirmed",
            Self::Failed => "Failed",
            Self::Abandoned => "Abandoned",
        }
    }

    /// True while the intent still needs driving to a terminal state.
    #[must_use]
    pub const fn is_open(self) -> bool {
        !matches!(self, Self::Confirmed | Self::Abandoned)
    }
}

/// Releases the ownership a previous run left on intents that never finished.
///
/// An intent owned by a process that is gone can never be picked up: the owner
/// column says someone is working on it and nobody is. Startup clears that
/// claim for its own instance, and for any claim whose lease has expired,
/// leaving the intent itself untouched in its recorded state.
///
/// It does not execute anything. Re-driving an intent takes the side effect
/// back to Plex or the filesystem, and startup's job here is to make that
/// possible, not to do it.
#[derive(Debug)]
pub struct ReleaseStaleIntents {
    /// The process instance whose claims are being released.
    pub instance_id: String,
    /// The instant of this start, against which leases are judged expired.
    pub at: Timestamp,
}

impl WriteOperation for ReleaseStaleIntents {
    type Output = u64;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<u64, sqlx::Error> {
        let owner = self.instance_id;
        let at = self.at.as_millis();
        let affected = sqlx::query!(
            "UPDATE lifecycle_intents SET owner = NULL, lease_expires_at = NULL
             WHERE state NOT IN ('Confirmed', 'Abandoned')
               AND (owner = ?1 OR lease_expires_at < ?2)
               AND (owner IS NOT NULL OR lease_expires_at IS NOT NULL)",
            owner,
            at
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();
        Ok(affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_confirmed_and_abandoned_are_closed() {
        assert!(IntentState::Intended.is_open());
        assert!(IntentState::Executing.is_open());
        assert!(IntentState::Executed.is_open());
        assert!(IntentState::Failed.is_open());
        assert!(!IntentState::Confirmed.is_open());
        assert!(!IntentState::Abandoned.is_open());
    }

    #[test]
    fn the_tokens_match_the_schema_check_exactly() {
        // The audit log is read by people and by tests; a case fold between the
        // documentation and the database is a translation layer nobody asked
        // for (PRD §19.1).
        assert_eq!(IntentState::Intended.as_token(), "Intended");
        assert_eq!(IntentState::Abandoned.as_token(), "Abandoned");
    }
}
