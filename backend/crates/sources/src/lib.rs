// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # afisharr-sources
//!
//! External source adapters, and the one client every outbound request goes
//! through.
//!
//! PRD §21.2.5 puts every outbound HTTP request — source adapters, the Plex
//! client, the `*arr` clients, artwork, the volatile-parameter feed, dataset
//! imports — behind a single client in this crate. Two properties follow that
//! are not achievable per adapter: every request is timed, including in
//! adapters written later, and every request carries a hard deadline set on
//! the client rather than passed at the call site.
//!
//! The adapters themselves, the rate limiters, the circuit breakers, and the
//! parser-versioned response cache arrive in Phase 5. [`outbound`] is the seam
//! they are built on, and it exists now because Phase 1's Plex login is
//! already an outbound request.

/// The version of this crate in the running build.
///
/// Reported in the boot log so a support bundle names every component present,
/// rather than one number that only describes the binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod outbound;
