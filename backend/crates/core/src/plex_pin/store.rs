// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `plex_pin_logins` table.

use sqlx::{SqliteConnection, SqlitePool};

use crate::{identifier::Id, storage::WriteOperation, time::Timestamp};

/// How a pin login ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinLoginResult {
    /// plex.tv issued a token.
    Success,
    /// The pin's window closed first.
    Expired,
    /// The operator or the interface abandoned it.
    Aborted,
}

impl PinLoginResult {
    /// The text stored in `plex_pin_logins.result`.
    #[must_use]
    pub const fn as_text(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::Expired => "Expired",
            Self::Aborted => "Aborted",
        }
    }
}

/// One in-flight or finished pin login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinLogin {
    /// Afisharr's identifier for this attempt, which is what the client polls.
    pub id: String,
    /// plex.tv's identifier for the pin.
    pub plex_pin_id: String,
    /// The link code, kept so the interface can redisplay it on a refresh.
    pub code: String,
    /// `Pin` or `OAuth`.
    pub mode: String,
    /// The client identifier the pin was created under.
    pub client_identifier: String,
    /// When the attempt started.
    pub created_at: Timestamp,
    /// When plex.tv stops answering for this pin.
    pub expires_at: Timestamp,
    /// When the attempt finished, if it has.
    pub consumed_at: Option<Timestamp>,
    /// How it finished, if it has.
    pub result: Option<String>,
}

impl PinLogin {
    /// Whether this attempt is still worth polling at `now`.
    #[must_use]
    pub fn is_open(&self, now: Timestamp) -> bool {
        self.consumed_at.is_none() && now < self.expires_at
    }
}

/// Reads one attempt.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn find(readers: &SqlitePool, id: &str) -> Result<Option<PinLogin>, sqlx::Error> {
    Ok(
        sqlx::query_as!(Row, "SELECT * FROM plex_pin_logins WHERE id = ?1", id)
            .fetch_optional(readers)
            .await?
            .map(PinLogin::from),
    )
}

/// Records a newly created pin.
#[derive(Debug)]
pub struct RecordPinLogin {
    /// The identifier to assign.
    pub id: Id,
    /// plex.tv's identifier for the pin.
    pub plex_pin_id: String,
    /// The link code.
    pub code: String,
    /// `Pin` or `OAuth`.
    pub mode: &'static str,
    /// The client identifier the pin was created under.
    pub client_identifier: String,
    /// When the attempt started.
    pub at: Timestamp,
    /// When plex.tv stops answering for this pin.
    pub expires_at: Timestamp,
}

impl WriteOperation for RecordPinLogin {
    type Output = PinLogin;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<PinLogin, sqlx::Error> {
        let id = self.id.as_str().to_owned();
        let at = self.at.as_millis();
        let expires_at = self.expires_at.as_millis();
        sqlx::query!(
            "INSERT INTO plex_pin_logins (
                 id, plex_pin_id, code, mode, client_identifier, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            id,
            self.plex_pin_id,
            self.code,
            self.mode,
            self.client_identifier,
            at,
            expires_at
        )
        .execute(&mut *conn)
        .await?;

        Ok(PinLogin::from(
            sqlx::query_as!(Row, "SELECT * FROM plex_pin_logins WHERE id = ?1", id)
                .fetch_one(&mut *conn)
                .await?,
        ))
    }
}

/// Claims an attempt for the request that is about to authorise it.
///
/// The one write that decides which of two overlapping polls proceeds. The
/// interface polls on a timer, so two requests can be in flight when the token
/// appears; both read the attempt as open, both are told `Authorized` by
/// plex.tv, and both mint a session unless something makes the transition
/// happen once. This is that something: a single `consumed_at IS NULL` update
/// through the serialised write actor (D-024), answering `true` exactly once.
///
/// The result is recorded afterwards by [`CompletePinLogin`], because how the
/// attempt ended is not known until the authorisation it guards has run.
#[derive(Debug)]
pub struct ClaimPinLogin {
    /// The attempt being claimed.
    pub id: String,
    /// The instant of the claim.
    pub at: Timestamp,
}

impl WriteOperation for ClaimPinLogin {
    type Output = bool;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
        let at = self.at.as_millis();
        let affected = sqlx::query!(
            "UPDATE plex_pin_logins SET consumed_at = ?2
             WHERE id = ?1 AND consumed_at IS NULL",
            self.id,
            at
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();
        Ok(affected == 1)
    }
}

/// Records how an attempt ended, closing it if it is not closed already.
///
/// Guarded on `result IS NULL` rather than on `consumed_at IS NULL`, so it can
/// finalise a row [`ClaimPinLogin`] has already stamped and still refuse to
/// overwrite an outcome that is recorded. Idempotent either way, and that
/// matters: the interface polls, and the second of two polls must not reopen
/// or relabel a login the first finished.
#[derive(Debug)]
pub struct CompletePinLogin {
    /// The attempt being closed.
    pub id: String,
    /// How it ended.
    pub result: PinLoginResult,
    /// The instant it ended.
    pub at: Timestamp,
}

impl WriteOperation for CompletePinLogin {
    type Output = bool;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
        let at = self.at.as_millis();
        let result = self.result.as_text();
        let affected = sqlx::query!(
            "UPDATE plex_pin_logins SET consumed_at = COALESCE(consumed_at, ?2), result = ?3
             WHERE id = ?1 AND result IS NULL",
            self.id,
            at,
            result
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();
        Ok(affected == 1)
    }
}

/// The `plex_pin_logins` row exactly as `SQLite` returns it.
struct Row {
    id: String,
    plex_pin_id: String,
    code: String,
    mode: String,
    client_identifier: String,
    created_at: i64,
    expires_at: i64,
    consumed_at: Option<i64>,
    result: Option<String>,
}

impl From<Row> for PinLogin {
    fn from(row: Row) -> Self {
        Self {
            id: row.id,
            plex_pin_id: row.plex_pin_id,
            code: row.code,
            mode: row.mode,
            client_identifier: row.client_identifier,
            created_at: Timestamp::from_millis(row.created_at),
            expires_at: Timestamp::from_millis(row.expires_at),
            consumed_at: row.consumed_at.map(Timestamp::from_millis),
            result: row.result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn login(consumed: Option<i64>, expires_at: i64) -> PinLogin {
        PinLogin {
            id: "01J".to_owned(),
            plex_pin_id: "42".to_owned(),
            code: "abcd".to_owned(),
            mode: "Pin".to_owned(),
            client_identifier: "01JCLIENT".to_owned(),
            created_at: Timestamp::EPOCH,
            expires_at: Timestamp::from_millis(expires_at),
            consumed_at: consumed.map(Timestamp::from_millis),
            result: None,
        }
    }

    #[test]
    fn an_unconsumed_unexpired_attempt_is_open() {
        assert!(login(None, 1_000).is_open(Timestamp::from_millis(999)));
    }

    #[test]
    fn an_attempt_closes_exactly_at_its_expiry() {
        assert!(!login(None, 1_000).is_open(Timestamp::from_millis(1_000)));
    }

    #[test]
    fn a_consumed_attempt_is_closed_however_much_time_is_left() {
        assert!(!login(Some(1), 1_000).is_open(Timestamp::from_millis(2)));
    }

    #[test]
    fn every_result_renders_the_text_the_schema_allows() {
        assert_eq!(PinLoginResult::Success.as_text(), "Success");
        assert_eq!(PinLoginResult::Expired.as_text(), "Expired");
        assert_eq!(PinLoginResult::Aborted.as_text(), "Aborted");
    }
}
