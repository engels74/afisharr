// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `sessions` table.

use sqlx::{SqliteConnection, SqlitePool};

use crate::{
    sessions::{ABSOLUTE_LIFETIME_MILLIS, Session},
    storage::WriteOperation,
    time::Timestamp,
};

/// Reads the session stored under this digest, if one exists.
///
/// The lookup takes a digest rather than a cookie value on purpose: a function
/// that accepts the plaintext is a function somebody logs the argument of.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn find_by_digest(
    readers: &SqlitePool,
    digest: &str,
) -> Result<Option<Session>, sqlx::Error> {
    Ok(
        sqlx::query_as!(Row, "SELECT * FROM sessions WHERE id = ?1", digest)
            .fetch_optional(readers)
            .await?
            .map(Session::from),
    )
}

/// Every session belonging to one account, newest first.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn list_for_user(
    readers: &SqlitePool,
    user_id: &str,
) -> Result<Vec<Session>, sqlx::Error> {
    Ok(sqlx::query_as!(
        Row,
        "SELECT * FROM sessions WHERE user_id = ?1 ORDER BY created_at DESC",
        user_id
    )
    .fetch_all(readers)
    .await?
    .into_iter()
    .map(Session::from)
    .collect())
}

/// How many unrevoked, unexpired sessions exist at `now`.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn count_active(readers: &SqlitePool, now: Timestamp) -> Result<i64, sqlx::Error> {
    let now = now.as_millis();
    sqlx::query_scalar!(
        "SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL AND expires_at > ?1",
        now
    )
    .fetch_one(readers)
    .await
}

/// Stores a new session under the digest of its cookie value.
#[derive(Debug)]
pub struct CreateSession {
    /// SHA-256 of the cookie value. Never the value.
    pub digest: String,
    /// The account signing in.
    pub user_id: String,
    /// The user agent that presented the credentials.
    pub user_agent: Option<String>,
    /// The peer address the credentials arrived from.
    pub ip: Option<String>,
    /// The instant of the sign-in.
    pub at: Timestamp,
}

impl WriteOperation for CreateSession {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let at = self.at.as_millis();
        let expires_at = self.at.plus_millis(ABSOLUTE_LIFETIME_MILLIS).as_millis();
        sqlx::query!(
            "INSERT INTO sessions (id, user_id, created_at, expires_at, last_seen_at,
                                   user_agent, ip)
             VALUES (?1, ?2, ?3, ?4, ?3, ?5, ?6)",
            self.digest,
            self.user_id,
            at,
            expires_at,
            self.user_agent,
            self.ip
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

/// Slides the idle window forward on a session that is still active.
///
/// The `WHERE` clause repeats the validity rules rather than trusting the
/// caller's read: a session revoked between the read and this write must not
/// be resurrected by the request that was already in flight.
#[derive(Debug)]
pub struct TouchSession {
    /// The session being used.
    pub digest: String,
    /// The instant of the request.
    pub at: Timestamp,
}

impl WriteOperation for TouchSession {
    type Output = bool;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
        let at = self.at.as_millis();
        let affected = sqlx::query!(
            "UPDATE sessions SET last_seen_at = ?2
             WHERE id = ?1 AND revoked_at IS NULL AND expires_at > ?2",
            self.digest,
            at
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();
        Ok(affected == 1)
    }
}

/// Revokes one session.
#[derive(Debug)]
pub struct RevokeSession {
    /// The session being revoked.
    pub digest: String,
    /// The instant of the revocation.
    pub at: Timestamp,
}

impl WriteOperation for RevokeSession {
    type Output = bool;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
        let at = self.at.as_millis();
        let affected = sqlx::query!(
            "UPDATE sessions SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
            self.digest,
            at
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();
        Ok(affected == 1)
    }
}

/// Revokes every session an account holds, and reports how many.
///
/// A password change runs this (PRD §21.4.2). `keep` names the session that
/// performed the change, so the operator who just rotated their own password
/// is not signed out of the tab they did it in — every other session, on every
/// other device, is gone.
#[derive(Debug)]
pub struct RevokeAllForUser {
    /// The account whose sessions are being revoked.
    pub user_id: String,
    /// A session digest to spare, if any.
    pub keep: Option<String>,
    /// The instant of the revocation.
    pub at: Timestamp,
}

impl WriteOperation for RevokeAllForUser {
    type Output = u64;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<u64, sqlx::Error> {
        let at = self.at.as_millis();
        // IFNULL rather than two statements: `keep` being absent must revoke
        // everything, and a digest never equals the empty string.
        let keep = self.keep.unwrap_or_default();
        let affected = sqlx::query!(
            "UPDATE sessions SET revoked_at = ?3
             WHERE user_id = ?1 AND revoked_at IS NULL AND id <> ?2",
            self.user_id,
            keep,
            at
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();
        Ok(affected)
    }
}

/// The `sessions` row exactly as `SQLite` returns it.
struct Row {
    id: String,
    user_id: String,
    created_at: i64,
    expires_at: i64,
    last_seen_at: i64,
    user_agent: Option<String>,
    ip: Option<String>,
    revoked_at: Option<i64>,
}

impl From<Row> for Session {
    fn from(row: Row) -> Self {
        Self {
            digest: row.id,
            user_id: row.user_id,
            created_at: Timestamp::from_millis(row.created_at),
            expires_at: Timestamp::from_millis(row.expires_at),
            last_seen_at: Timestamp::from_millis(row.last_seen_at),
            user_agent: row.user_agent,
            ip: row.ip,
            revoked_at: row.revoked_at.map(Timestamp::from_millis),
        }
    }
}
