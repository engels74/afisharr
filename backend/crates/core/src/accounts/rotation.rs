// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Changing a password and replacing every session it protected, atomically.

use sqlx::SqliteConnection;

use crate::{sessions::ABSOLUTE_LIFETIME_MILLIS, storage::WriteOperation, time::Timestamp};

/// What a rotation did.
///
/// Values rather than errors for the two refusals: the caller re-verified the
/// current password before submitting this, so finding nothing to change means
/// the account was deleted or converted in between, or somebody else's change
/// landed first. Both are refusals to render, and neither is a fault to report.
#[derive(Debug, PartialEq, Eq)]
pub enum PasswordRotation {
    /// The password changed, and this many sessions *other than the caller's*
    /// were revoked.
    Rotated {
        /// How many other devices were signed out.
        others_revoked: u64,
    },
    /// The password had already moved on, so nothing was written.
    ///
    /// Two requests that verified the same current password are two rotations
    /// of one credential. The second is stale by the time it arrives, and the
    /// account keeps the first one's password and the first one's replacement
    /// session.
    Superseded,
    /// No enabled local account holds that identifier, so nothing was written.
    NoLocalAccount,
}

/// Rotates one account's password and re-issues the session that asked.
///
/// The three writes are one transaction because the guarantee is the
/// conjunction of them, not any one on its own: a password that changed while
/// the old session identifiers stayed valid is a rotation that ends nothing —
/// including the theft it was performed to end (PRD §21.4.2). Anything that
/// stops the process between two separate writes leaves exactly that state, so
/// there is nothing between them to stop.
///
/// The replacement session is inserted here rather than by a later call for the
/// same reason. The caller must not be handed a cookie for a session that a
/// crash could roll back, so the cookies are attached only after this commits.
#[derive(Debug)]
pub struct RotatePassword {
    /// The account being changed.
    pub user_id: String,
    /// The Argon2id PHC string the caller verified against.
    ///
    /// The rotation is conditional on it. Without it the write is
    /// last-one-wins: two changes that verified the same password would both
    /// commit, and the second would revoke the replacement session the first
    /// one's browser is holding.
    pub expected_hash: String,
    /// The new Argon2id PHC string.
    pub password_hash: String,
    /// The session that asked, revoked with the rest and not counted with them.
    ///
    /// `None` when the caller presented an API key, which is not a session and
    /// leaves nothing of its own to rotate away.
    pub current_session: Option<String>,
    /// SHA-256 of the replacement session's cookie value. Never the value.
    pub replacement_digest: String,
    /// The user agent that asked for the change.
    pub user_agent: Option<String>,
    /// The address it was asked from.
    pub ip: Option<String>,
    /// The instant of the write.
    pub at: Timestamp,
}

impl WriteOperation for RotatePassword {
    type Output = PasswordRotation;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<Self::Output, sqlx::Error> {
        let at = self.at.as_millis();
        let expires_at = self.at.plus_millis(ABSOLUTE_LIFETIME_MILLIS).as_millis();
        // A digest never equals the empty string, so an API-key caller with no
        // session of its own spares nothing and revokes everything.
        let asking = self.current_session.unwrap_or_default();

        let mut transaction = sqlx::Connection::begin(conn).await?;

        let changed = sqlx::query!(
            "UPDATE users SET password_hash = ?2, updated_at = ?3
             WHERE id = ?1 AND kind = 'Local' AND disabled_at IS NULL
               AND password_hash = ?4",
            self.user_id,
            self.password_hash,
            at,
            self.expected_hash
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if changed != 1 {
            // Two changes that verified the same password are two rotations of
            // one credential, and the second of them is rotating away a
            // password that is already gone. Letting it write would revoke the
            // first one's replacement session — leaving a browser holding a
            // cookie the instance no longer honours — and hand the rotation to
            // whichever caller committed last (PRD §21.4.2).
            let present = sqlx::query_scalar!(
                "SELECT 1 FROM users
                 WHERE id = ?1 AND kind = 'Local' AND disabled_at IS NULL",
                self.user_id
            )
            .fetch_optional(&mut *transaction)
            .await?
            .is_some();
            transaction.rollback().await?;
            return Ok(if present {
                PasswordRotation::Superseded
            } else {
                PasswordRotation::NoLocalAccount
            });
        }

        // Counted apart from the caller's own so the answer can say how many
        // *other* devices were signed out without arithmetic that guesses
        // whether the caller had a session at all.
        let others_revoked = sqlx::query!(
            "UPDATE sessions SET revoked_at = ?3
             WHERE user_id = ?1 AND revoked_at IS NULL AND id <> ?2",
            self.user_id,
            asking,
            at
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();

        sqlx::query!(
            "UPDATE sessions SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
            asking,
            at
        )
        .execute(&mut *transaction)
        .await?;

        sqlx::query!(
            "INSERT INTO sessions (id, user_id, created_at, expires_at, last_seen_at,
                                   user_agent, ip)
             VALUES (?1, ?2, ?3, ?4, ?3, ?5, ?6)",
            self.replacement_digest,
            self.user_id,
            at,
            expires_at,
            self.user_agent,
            self.ip
        )
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(PasswordRotation::Rotated { others_revoked })
    }
}
