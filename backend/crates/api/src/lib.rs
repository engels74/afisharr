// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # afisharr-api
//!
//! The HTTP surface.
//!
//! Axum routes, auth and sessions, SSE, and the utoipa-generated `OpenAPI`
//! document that the TypeScript client is generated from.
//!
//! Built in Phase 1; this crate carries only its identity until then.

/// The version of this crate in the running build.
///
/// Reported in the boot log so a support bundle names every component present,
/// rather than one number that only describes the binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
