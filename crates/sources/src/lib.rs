// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # afisharr-sources
//!
//! External source adapters.
//!
//! One module per provider behind a common `SourceBuilder` trait, each with a
//! typed client, rate limiter, circuit breaker, response validation, and health
//! status. Every outbound request in the product goes through the single
//! instrumented client this crate owns (PRD §21.2.5).
//!
//! Built in Phase 5; this crate carries only its identity until then.

/// The version of this crate in the running build.
///
/// Reported in the boot log so a support bundle names every component present,
/// rather than one number that only describes the binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
