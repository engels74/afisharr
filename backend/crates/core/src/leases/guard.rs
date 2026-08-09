// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A held lease, and the heartbeat that proves it is still held.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{
    leases::{Acquire, Heartbeat, LeaseError, LeaseName, LeaseOwner, Release, held_by},
    storage::WriteHandle,
    time::Clock,
};

/// A lease this process currently holds.
///
/// The guard does not renew itself on a timer. A pass calls
/// [`LeaseGuard::heartbeat`] at its own checkpoints, and a `Err(LeaseError::Lost)`
/// there is the instruction to abort: another holder may already have started,
/// so finishing would be two passes completing over one scope.
#[derive(Debug)]
pub struct LeaseGuard {
    name: LeaseName,
    owner: LeaseOwner,
    writer: WriteHandle,
    clock: Arc<dyn Clock>,
    ttl_millis: i64,
}

impl LeaseGuard {
    /// Takes `name` for `owner`, stealing it only if the current claim expired.
    ///
    /// # Errors
    /// Returns [`LeaseError::Held`] when a live claim exists, naming who holds
    /// it and until when.
    pub async fn acquire(
        writer: &WriteHandle,
        readers: &SqlitePool,
        clock: Arc<dyn Clock>,
        name: LeaseName,
        owner: LeaseOwner,
        ttl_millis: i64,
    ) -> Result<Self, LeaseError> {
        let now = clock.now();
        let acquired = writer
            .submit(Acquire {
                name: name.clone(),
                owner: owner.clone(),
                at: now,
                expires_at: now.plus_millis(ttl_millis),
            })
            .await?;

        if acquired {
            return Ok(Self {
                name,
                owner,
                writer: writer.clone(),
                clock,
                ttl_millis,
            });
        }

        // Name the current holder rather than reporting a bare refusal: an
        // operator who is told "held" and not "by what" has to go looking.
        let holder = held_by(readers, &name)
            .await
            .map_err(crate::storage::StorageError::from)?;
        Err(LeaseError::Held {
            name,
            holder: holder
                .as_ref()
                .map_or_else(|| "unknown".to_owned(), |h| h.owner.clone()),
            expires_at: holder.map_or(0, |h| h.expires_at.as_millis()),
        })
    }

    /// The lease's name.
    #[must_use]
    pub fn name(&self) -> &LeaseName {
        &self.name
    }

    /// Renews the claim.
    ///
    /// # Errors
    /// Returns [`LeaseError::Lost`] when the row has gone or now names someone
    /// else. The caller must abort the pass.
    pub async fn heartbeat(&self) -> Result<(), LeaseError> {
        let now = self.clock.now();
        let still_held = self
            .writer
            .submit(Heartbeat {
                name: self.name.clone(),
                owner: self.owner.clone(),
                at: now,
                expires_at: now.plus_millis(self.ttl_millis),
            })
            .await?;

        if still_held {
            Ok(())
        } else {
            Err(LeaseError::Lost {
                name: self.name.clone(),
            })
        }
    }

    /// Gives the lease up.
    ///
    /// # Errors
    /// Returns [`LeaseError::Storage`] if the delete fails.
    pub async fn release(self) -> Result<(), LeaseError> {
        self.writer
            .submit(Release {
                name: self.name.clone(),
                owner: self.owner.clone(),
            })
            .await?;
        Ok(())
    }
}
