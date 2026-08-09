// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Undoing what a previous run left claimed.

use afisharr_core::{
    leases::ClearOwnedBy, lifecycle::ReleaseStaleIntents, storage::WriteHandle, time::Timestamp,
};
use anyhow::{Context, Result};
use tracing::info;

/// What the final startup step released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reconciled {
    /// Leases this instance held before the restart.
    pub leases_cleared: u64,
    /// Open intents whose ownership was released.
    pub intents_released: u64,
}

/// Clears this instance's leases, then releases ownership of open intents.
///
/// Leases first, and before any pass runs: a pass that started before the
/// reconcile finished could take a lease that is then cleared underneath it,
/// which is the two-passes-at-once failure the lease exists to prevent.
///
/// Leases owned by *another* process are left alone. They may belong to a
/// process that is still alive, and an expired one is stolen at acquisition
/// anyway — deleting it here would buy nothing and could cut across a live run.
#[tracing::instrument(skip(writer))]
pub async fn run(writer: &WriteHandle, instance_id: &str, at: Timestamp) -> Result<Reconciled> {
    let leases_cleared = writer
        .submit(ClearOwnedBy {
            instance_id: instance_id.to_owned(),
        })
        .await
        .context("clearing leases held by this instance before the restart")?;

    let intents_released = writer
        .submit(ReleaseStaleIntents {
            instance_id: instance_id.to_owned(),
            at,
        })
        .await
        .context("releasing ownership of intents left open by a previous run")?;

    if leases_cleared > 0 || intents_released > 0 {
        info!(
            leases_cleared,
            intents_released, "reconciled state left by a previous run"
        );
    }

    Ok(Reconciled {
        leases_cleared,
        intents_released,
    })
}
