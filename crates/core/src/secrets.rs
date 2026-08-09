// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Credentials at rest.
//!
//! D-032 settles the shape: XChaCha20-Poly1305 with a random nonce per secret,
//! and a 32-byte key from the OS CSPRNG stored beside the database at
//! `secrets.key` with mode `0600`, overridable by `AFISHARR_SECRET_KEY`.
//!
//! What this protects against, stated honestly (PRD §21.4.5): a stolen database
//! file. It does not protect against an attacker who can read the filesystem,
//! because such an attacker reads the key too. That is the standard limit of
//! encryption at rest for an unattended service, and it is written down here
//! rather than implied.

mod cipher;
mod error;
mod key_file;
mod store;

pub use cipher::{ALGORITHM, Sealed, SecretKey};
pub use error::SecretError;
pub use key_file::{KEY_ENV_VAR, load_or_create};
pub use store::{PutSecret, get};
