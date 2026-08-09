// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Who may sign in, and what proves it.
//!
//! Two kinds of account share one table (PRD §19.6): a local account whose
//! proof is an Argon2id PHC string, and a Plex account whose proof is a token
//! plex.tv issued. The password half never leaves [`password`] — a second
//! hashing call site is a second parameter set, and the one that drifts is the
//! one nobody measured.

mod error;
mod password;
mod store;
mod user;

pub use error::AccountError;
pub use password::{PARAMETERS, hash, verify};
pub use store::{
    CreateUser, CreateUserOutcome, SetPassword, TouchLastLogin, UpsertPlexUser, admin_exists,
    find_by_id, find_by_plex_account, find_by_username,
};
pub use user::{User, UserKind};
