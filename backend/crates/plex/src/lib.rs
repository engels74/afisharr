// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # afisharr-plex
//!
//! The Plex client.
//!
//! Libraries, collections, hubs, labels, media streams, artwork, and
//! filter-metadata discovery.
//!
//! Built in Phase 2; this crate carries only its identity until then.

/// The version of this crate in the running build.
///
/// Reported in the boot log so a support bundle names every component present,
/// rather than one number that only describes the binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
