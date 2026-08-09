// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What can go wrong reading or writing settings.

use thiserror::Error;

use crate::storage::StorageError;

/// A failure loading or saving the settings document.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SettingsError {
    /// The stored body did not deserialise into the typed document.
    #[error("the stored settings body is not a valid settings document")]
    Malformed(#[source] serde_json::Error),

    /// The database refused the statement.
    #[error("settings storage failed")]
    Storage(#[from] StorageError),
}
