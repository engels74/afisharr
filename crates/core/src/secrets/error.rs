// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What can go wrong handling a secret.

use std::path::PathBuf;

use thiserror::Error;

use crate::storage::StorageError;

/// A failure reading, writing, or decrypting a secret.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SecretError {
    /// The key file could not be read or written.
    #[error("the secret key file at {path} could not be used")]
    KeyFile {
        /// Where the key was expected.
        path: PathBuf,
        /// The underlying failure.
        #[source]
        source: std::io::Error,
    },

    /// The key material was not 32 bytes.
    #[error("{source_name} holds {found} bytes; a key is 32")]
    KeyLength {
        /// Where the material came from — the file path or the variable name.
        source_name: String,
        /// How many bytes were found.
        found: usize,
    },

    /// `AFISHARR_SECRET_KEY` was set to something other than 64 hex characters,
    /// including a value this process cannot read as text.
    #[error("AFISHARR_SECRET_KEY must be 64 lowercase hex characters")]
    KeyEncoding,

    /// The stored row names an algorithm this binary does not implement.
    #[error("secret '{name}' is sealed with unsupported algorithm '{algorithm}'")]
    UnsupportedAlgorithm {
        /// The secret that could not be opened.
        name: String,
        /// The algorithm the row names.
        algorithm: String,
    },

    /// The ciphertext did not authenticate under this key.
    ///
    /// The usual cause is a database restored without its `secrets.key`
    /// (PRD §21.6.3). That is a secret whose value is *unobservable*, not a
    /// secret that is absent, and nothing may be deleted on the strength of it.
    #[error("secret '{name}' could not be decrypted with the current key")]
    Undecryptable {
        /// The secret that could not be opened.
        name: String,
    },

    /// The OS entropy source was unavailable.
    #[error("the OS CSPRNG is unavailable")]
    Entropy,

    /// The database refused the statement.
    #[error("secret storage failed")]
    Storage(#[from] StorageError),
}
