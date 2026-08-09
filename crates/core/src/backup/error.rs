// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What can go wrong taking a backup.

use std::path::PathBuf;

use thiserror::Error;

/// A failure copying the database or pruning old copies.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BackupError {
    /// The backup directory could not be created or listed.
    #[error("the backup directory {path} could not be used")]
    Directory {
        /// The directory in question.
        path: PathBuf,
        /// The underlying failure.
        #[source]
        source: std::io::Error,
    },

    /// `SQLite` refused to open the source or destination, or the copy failed.
    ///
    /// A failed backup blocks the migration it precedes (`I-DATA-8`). The
    /// alternative — migrating anyway — is a forward-only migration with no
    /// recovery path, which is the trade forward-only makes and only survives
    /// if the backup is real.
    #[error("copying {from} to {to} through the online backup API")]
    Copy {
        /// The database being copied.
        from: PathBuf,
        /// Where the copy was going.
        to: PathBuf,
        /// The underlying failure.
        #[source]
        source: rusqlite::Error,
    },

    /// The blocking copy task did not finish.
    #[error("the backup task did not complete")]
    TaskFailed,
}
