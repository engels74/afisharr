// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What can go wrong holding a lease.

use thiserror::Error;

use crate::{leases::LeaseName, storage::StorageError};

/// A failure acquiring, holding, or releasing a lease.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LeaseError {
    /// Another holder has the lease and it has not expired.
    #[error("{name} is held by {holder} until {expires_at}")]
    Held {
        /// The lease that was wanted.
        name: LeaseName,
        /// Who holds it.
        holder: String,
        /// When their claim lapses, in epoch milliseconds.
        expires_at: i64,
    },

    /// The lease was taken by someone else while this pass was working.
    ///
    /// The pass must abort rather than complete: another holder may already
    /// have started, and two passes finishing is the thing the lease prevents.
    #[error("{name} was lost while the pass was running")]
    Lost {
        /// The lease that is no longer held.
        name: LeaseName,
    },

    /// The database refused the statement.
    #[error("lease storage failed")]
    Storage(#[from] StorageError),
}
