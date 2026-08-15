// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The Plex connection, as Settings sees it.
//!
//! One question — is the server this installation is bound to the server that
//! answers at its address? — and six answers, because collapsing them loses
//! exactly the distinctions the operator needs. "Nothing is configured", "no
//! credential", "the credential was refused", "it did not answer", and
//! "something else answered" are five different problems with five different
//! fixes, and the sixth answer is that it is working (P1, PRD §8.1).
//!
//! No library content is behind any of it. Phase 2 binds identity and nothing
//! more: a blocked connection must be visible before anything reads a rating
//! key, because every rating key in the database belongs to one server
//! (`I-ID-5`).

mod answer;
mod check;
mod connection;
pub(crate) mod routes;
mod shown;

pub use connection::{PlexConnection, PlexConnectionState};
pub use routes::check_connection;
