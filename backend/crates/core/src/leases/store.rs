// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `leases` table: acquire, heartbeat, release, and startup cleanup.

use sqlx::{SqliteConnection, SqlitePool};

use crate::{
    leases::{LeaseName, LeaseOwner},
    storage::WriteOperation,
    time::Timestamp,
};

/// Who holds a lease, and until when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    /// The value in `leases.owner`.
    pub owner: String,
    /// When the claim lapses.
    pub expires_at: Timestamp,
}

/// Reads the current holder of `name`, if any.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn held_by(
    readers: &SqlitePool,
    name: &LeaseName,
) -> Result<Option<Holder>, sqlx::Error> {
    let key = name.as_text();
    let row = sqlx::query!("SELECT owner, expires_at FROM leases WHERE name = ?1", key)
        .fetch_optional(readers)
        .await?;
    Ok(row.map(|row| Holder {
        owner: row.owner,
        expires_at: Timestamp::from_millis(row.expires_at),
    }))
}

/// Takes `name` for `owner` until `expires_at`, stealing only an expired lease.
///
/// One conditional insert-or-update, which is atomic because the write actor
/// serialises it (PRD §19.4). Returns `true` when the lease is now held.
#[derive(Debug)]
pub struct Acquire {
    /// The lease being taken.
    pub name: LeaseName,
    /// Who is taking it.
    pub owner: LeaseOwner,
    /// The instant the claim starts.
    pub at: Timestamp,
    /// The instant the claim lapses if no heartbeat renews it.
    pub expires_at: Timestamp,
}

impl WriteOperation for Acquire {
    type Output = bool;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
        let name = self.name.as_text();
        let owner = self.owner.as_str().to_owned();
        let at = self.at.as_millis();
        let expires_at = self.expires_at.as_millis();
        let affected = sqlx::query!(
            "INSERT INTO leases (name, owner, acquired_at, expires_at, heartbeat_at)
             VALUES (?1, ?2, ?3, ?4, ?3)
             ON CONFLICT(name) DO UPDATE SET
                 owner = excluded.owner, acquired_at = excluded.acquired_at,
                 expires_at = excluded.expires_at, heartbeat_at = excluded.heartbeat_at
             WHERE leases.expires_at < ?3",
            name,
            owner,
            at,
            expires_at
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();
        Ok(affected == 1)
    }
}

/// Renews a lease this owner still holds, and reports whether it still holds it.
///
/// Returns `false` when the row has gone or names someone else — the signal a
/// long pass must abort on.
#[derive(Debug)]
pub struct Heartbeat {
    /// The lease being renewed.
    pub name: LeaseName,
    /// Who believes they hold it.
    pub owner: LeaseOwner,
    /// The instant of this heartbeat.
    pub at: Timestamp,
    /// The instant the renewed claim lapses.
    pub expires_at: Timestamp,
}

impl WriteOperation for Heartbeat {
    type Output = bool;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
        let name = self.name.as_text();
        let owner = self.owner.as_str().to_owned();
        let at = self.at.as_millis();
        let expires_at = self.expires_at.as_millis();
        let affected = sqlx::query!(
            "UPDATE leases SET heartbeat_at = ?3, expires_at = ?4
             WHERE name = ?1 AND owner = ?2",
            name,
            owner,
            at,
            expires_at
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();
        Ok(affected == 1)
    }
}

/// Gives up a lease this owner holds. Releasing one held by someone else is a
/// no-op rather than an error: the pass has already lost it.
#[derive(Debug)]
pub struct Release {
    /// The lease being released.
    pub name: LeaseName,
    /// Who is releasing it.
    pub owner: LeaseOwner,
}

impl WriteOperation for Release {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let name = self.name.as_text();
        let owner = self.owner.as_str().to_owned();
        sqlx::query!(
            "DELETE FROM leases WHERE name = ?1 AND owner = ?2",
            name,
            owner
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

/// Clears every lease owned by this process instance, and reports how many.
///
/// Run at startup, before anything else touches the database: these leases are
/// ours from before the crash. Leases owned by another process are left to
/// expire on their own, because that process may still be alive. The setup
/// claim is never matched — its owner is a cookie digest, not an instance
/// prefix — so an interrupted setup survives a restart (D-046).
#[derive(Debug)]
pub struct ClearOwnedBy {
    /// The process instance whose leases are being cleared.
    pub instance_id: String,
}

impl WriteOperation for ClearOwnedBy {
    type Output = u64;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<u64, sqlx::Error> {
        let prefix = format!("{}%", LeaseOwner::instance_prefix(&self.instance_id));
        let affected = sqlx::query!("DELETE FROM leases WHERE owner LIKE ?1", prefix)
            .execute(&mut *conn)
            .await?
            .rows_affected();
        Ok(affected)
    }
}
