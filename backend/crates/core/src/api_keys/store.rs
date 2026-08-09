// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `api_keys` table.

use sqlx::{SqliteConnection, SqlitePool};

use crate::{api_keys::ApiKeyRecord, identifier::Id, storage::WriteOperation, time::Timestamp};

/// Reads the key stored under this digest, if one exists.
///
/// Returns the record even when it is revoked. The caller decides what a
/// revoked key means; reporting it as absent would make "revoked" and "never
/// issued" the same answer, and only one of those is worth logging.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn find_by_digest(
    readers: &SqlitePool,
    digest: &str,
) -> Result<Option<ApiKeyRecord>, sqlx::Error> {
    Ok(
        sqlx::query_as!(Row, "SELECT id, name, prefix, created_at, created_by, last_used_at, revoked_at FROM api_keys WHERE key_hash = ?1", digest)
            .fetch_optional(readers)
            .await?
            .map(ApiKeyRecord::from),
    )
}

/// Every key, newest first.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn list(readers: &SqlitePool) -> Result<Vec<ApiKeyRecord>, sqlx::Error> {
    Ok(
        sqlx::query_as!(Row, "SELECT id, name, prefix, created_at, created_by, last_used_at, revoked_at FROM api_keys ORDER BY created_at DESC")
            .fetch_all(readers)
            .await?
            .into_iter()
            .map(ApiKeyRecord::from)
            .collect(),
    )
}

/// Stores a new key under the digest of its plaintext.
#[derive(Debug)]
pub struct CreateApiKey {
    /// The identifier to assign.
    pub id: Id,
    /// The name the operator gave it.
    pub name: String,
    /// SHA-256 of the plaintext. Never the plaintext.
    pub digest: String,
    /// The leading characters, for display.
    pub prefix: String,
    /// The account issuing the key.
    pub created_by: Option<String>,
    /// The instant of the write.
    pub at: Timestamp,
}

impl WriteOperation for CreateApiKey {
    type Output = ApiKeyRecord;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<ApiKeyRecord, sqlx::Error> {
        let id = self.id.as_str().to_owned();
        let at = self.at.as_millis();
        sqlx::query!(
            "INSERT INTO api_keys (id, name, key_hash, prefix, created_at, created_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            id,
            self.name,
            self.digest,
            self.prefix,
            at,
            self.created_by
        )
        .execute(&mut *conn)
        .await?;

        Ok(ApiKeyRecord::from(
            sqlx::query_as!(Row, "SELECT id, name, prefix, created_at, created_by, last_used_at, revoked_at FROM api_keys WHERE id = ?1", id)
                .fetch_one(&mut *conn)
                .await?,
        ))
    }
}

/// Records that a key authenticated a request.
#[derive(Debug)]
pub struct TouchApiKey {
    /// The key that was accepted.
    pub id: String,
    /// The instant of the request.
    pub at: Timestamp,
}

impl WriteOperation for TouchApiKey {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let at = self.at.as_millis();
        sqlx::query!(
            "UPDATE api_keys SET last_used_at = ?2 WHERE id = ?1",
            self.id,
            at
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

/// Revokes one key. A revoked key is refused on its next use.
#[derive(Debug)]
pub struct RevokeApiKey {
    /// The key being revoked.
    pub id: String,
    /// The instant of the revocation.
    pub at: Timestamp,
}

impl WriteOperation for RevokeApiKey {
    type Output = bool;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
        let at = self.at.as_millis();
        let affected = sqlx::query!(
            "UPDATE api_keys SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
            self.id,
            at
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();
        Ok(affected == 1)
    }
}

/// The `api_keys` row exactly as `SQLite` returns it.
struct Row {
    id: String,
    name: String,
    prefix: String,
    created_at: i64,
    created_by: Option<String>,
    last_used_at: Option<i64>,
    revoked_at: Option<i64>,
}

impl From<Row> for ApiKeyRecord {
    fn from(row: Row) -> Self {
        Self {
            id: row.id,
            name: row.name,
            prefix: row.prefix,
            created_at: Timestamp::from_millis(row.created_at),
            created_by: row.created_by,
            last_used_at: row.last_used_at.map(Timestamp::from_millis),
            revoked_at: row.revoked_at.map(Timestamp::from_millis),
        }
    }
}
