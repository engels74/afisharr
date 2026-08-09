// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # afisharr-plex
//!
//! The Plex client: the protocol surface Afisharr actually calls, and no more.
//!
//! Phase 1 needs one part of it — the plex.tv PIN and OAuth token exchange
//! behind the login flow — so [`pin`] is what exists here. The library, item,
//! collection, hub, label, artwork, and filter-metadata calls arrive with the
//! rest of the protocol surface, along with the adversarial fake they are
//! tested against (D-036).
//!
//! Every request leaves through the one instrumented outbound client in
//! `afisharr-sources` (PRD §21.2.5). This crate owns the protocol; it does not
//! own a transport.

/// The version of this crate in the running build.
///
/// Reported in the boot log so a support bundle names every component present,
/// rather than one number that only describes the binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod identity;
pub mod pin;
