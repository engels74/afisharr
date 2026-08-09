// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What can go wrong reaching the database.

use std::path::PathBuf;

use thiserror::Error;

/// A failure reaching or mutating the database.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StorageError {
    /// A statement failed.
    #[error("database statement failed")]
    Statement(#[from] sqlx::Error),

    /// The database file, or the directory holding it, could not be opened.
    #[error("opening the database at {path}")]
    Open {
        /// The path that was being opened.
        path: PathBuf,
        /// The underlying failure.
        #[source]
        source: sqlx::Error,
    },

    /// The directory that should hold the database could not be created.
    #[error("creating the directory for {path}")]
    Directory {
        /// The database path whose parent directory was missing.
        path: PathBuf,
        /// The underlying failure.
        #[source]
        source: std::io::Error,
    },

    /// The write actor has stopped, so no further mutation can be accepted.
    ///
    /// Reached only during shutdown or after the actor's task panicked; either
    /// way the caller's mutation did not happen and must not be assumed to have.
    #[error("the write actor is no longer accepting mutations")]
    WriterStopped,
}
