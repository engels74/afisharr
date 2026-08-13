// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The Plex connection, as Settings sees it.
//!
//! One question — is the server this installation is bound to the server that
//! answers at its address? — and five answers, because collapsing them loses
//! exactly the distinctions the operator needs. "Nothing is configured", "no
//! credential", "it did not answer", and "something else answered" are four
//! different problems with four different fixes, and the fifth is that it is
//! working (P1, PRD §8.1).
//!
//! No library content is behind any of it. Phase 2 binds identity and nothing
//! more: a blocked connection must be visible before anything reads a rating
//! key, because every rating key in the database belongs to one server
//! (`I-ID-5`).

mod check;
mod connection;
pub(crate) mod routes;

pub use connection::{PlexConnection, PlexConnectionState};
pub use routes::check_connection;
