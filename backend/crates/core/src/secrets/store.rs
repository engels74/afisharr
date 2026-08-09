// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `secrets` table.

use sqlx::{SqliteConnection, SqlitePool};

use crate::{
    secrets::{Sealed, SecretError, SecretKey},
    storage::WriteOperation,
    time::Timestamp,
};

/// Reads and decrypts one secret.
///
/// Returns `Ok(None)` only when no row exists. A row that will not decrypt is
/// [`SecretError::Undecryptable`], never `None`: a database restored without its
/// key file holds secrets whose values are unobservable, and reporting them as
/// absent is failure pattern P1.
///
/// # Errors
/// Returns [`SecretError::Undecryptable`] when the ciphertext does not
/// authenticate, and [`SecretError::Storage`] when the read fails.
pub async fn get(
    readers: &SqlitePool,
    key: &SecretKey,
    name: &str,
) -> Result<Option<Vec<u8>>, SecretError> {
    let row = sqlx::query!(
        "SELECT ciphertext, nonce, algorithm FROM secrets WHERE name = ?1",
        name
    )
    .fetch_optional(readers)
    .await
    .map_err(|source| SecretError::Storage(source.into()))?;

    row.map(|row| {
        key.open(
            name,
            &Sealed {
                ciphertext: row.ciphertext,
                nonce: row.nonce,
                algorithm: row.algorithm,
            },
        )
    })
    .transpose()
}

/// Stores one secret, sealed under the instance key.
#[derive(Debug)]
pub struct PutSecret {
    /// The secret's name, e.g. `plex.token`.
    pub name: String,
    /// The sealed value.
    pub sealed: Sealed,
    /// The instant of the write.
    pub at: Timestamp,
}

impl WriteOperation for PutSecret {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let Self { name, sealed, at } = self;
        let at = at.as_millis();
        sqlx::query!(
            "INSERT INTO secrets (name, ciphertext, nonce, algorithm, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(name) DO UPDATE SET
                 ciphertext = excluded.ciphertext, nonce = excluded.nonce,
                 algorithm = excluded.algorithm, updated_at = excluded.updated_at",
            name,
            sealed.ciphertext,
            sealed.nonce,
            sealed.algorithm,
            at
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}
