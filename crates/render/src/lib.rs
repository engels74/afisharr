// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # afisharr-render
//!
//! The poster and overlay renderer.
//!
//! Element model, layers, font handling, and the content-addressed render cache.
//!
//! Built in Phase 8; this crate carries only its identity until then.

/// The version of this crate in the running build.
///
/// Reported in the boot log so a support bundle names every component present,
/// rather than one number that only describes the binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
