// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What can go wrong projecting a body onto its derived columns.

use thiserror::Error;

use crate::storage::StorageError;

/// A body that cannot be projected, or a sweep that could not run.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProjectionError {
    /// The stored body is not JSON.
    #[error("row {id}: the stored body is not JSON")]
    NotJson {
        /// The row that could not be read.
        id: String,
        /// The underlying parse failure.
        #[source]
        source: serde_json::Error,
    },

    /// The body is JSON but is missing a field every derived column needs.
    ///
    /// Reported rather than defaulted: a definition with no `kind` is a corrupt
    /// row, and writing `""` into the indexed column hides it behind a value
    /// that looks like data.
    #[error("row {id}: the envelope has no {pointer}")]
    MissingField {
        /// The row that could not be read.
        id: String,
        /// The JSON pointer the projection expected.
        pointer: String,
    },

    /// The envelope holds a value outside the set the column allows.
    #[error("row {id}: {pointer} is '{found}', which is not one of {expected}")]
    UnexpectedValue {
        /// The row that could not be read.
        id: String,
        /// The JSON pointer that held the value.
        pointer: String,
        /// What was there.
        found: String,
        /// What the column accepts.
        expected: String,
    },

    /// The database refused the statement.
    #[error("reprojection storage failed")]
    Storage(#[from] StorageError),
}
