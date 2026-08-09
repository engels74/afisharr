// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Browser sessions, stored as digests rather than as values.
//!
//! `sessions.id` is the SHA-256 of the cookie value and never the value
//! (PRD §19.6), so a database read yields nothing anyone can sign in with.
//! Two timeouts bound a session: seven days idle, sliding on every request,
//! and thirty days absolute with no extension.

mod lifetime;
mod store;
mod token;

pub use lifetime::{ABSOLUTE_LIFETIME_MILLIS, IDLE_TIMEOUT_MILLIS, Session, Validity};
pub use store::{
    CreateSession, RevokeAllForUser, RevokeSession, TouchSession, count_active, find_by_digest,
    list_for_user,
};
pub use token::SessionToken;
