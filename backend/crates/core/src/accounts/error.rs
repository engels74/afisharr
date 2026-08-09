// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What can go wrong reading or writing an account.

use thiserror::Error;

use crate::storage::StorageError;

/// A failure hashing, reading, or writing an account.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AccountError {
    /// The Argon2id parameters were rejected, or a stored hash will not parse.
    ///
    /// Carries the underlying message as text rather than the source error:
    /// `password_hash::Error` is not `std::error::Error` in this version, and a
    /// wrapper type that exists only to satisfy `#[source]` would say less.
    #[error("password hashing failed: {0}")]
    Hashing(String),

    /// The blocking hash task did not finish.
    #[error("the password hashing task did not complete")]
    Interrupted,

    /// A row holds a `kind` this binary does not know.
    #[error("account {id} holds an unknown kind '{kind}'")]
    UnknownKind {
        /// The row that could not be read.
        id: String,
        /// The value the column held.
        kind: String,
    },

    /// The database refused the statement.
    #[error("account storage failed")]
    Storage(#[from] StorageError),
}
