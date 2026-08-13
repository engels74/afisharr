// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # afisharr-api
//!
//! The HTTP surface: routing, one error shape, auth and sessions, the SSE
//! stream, and the `OpenAPI` document the TypeScript client is generated from.
//!
//! Two rules govern everything here.
//!
//! **Every failure is the same shape.** [`error::AppError`] is the only thing a
//! handler returns on the unhappy path, and it renders as
//! [`error::Problem`] — a code, a message, an optional JSON pointer into the
//! request body, and the expected-versus-actual pair when there is one. A
//! handler that invented its own status-code tuple would be a shape the
//! generated client does not know about.
//!
//! **The annotations are the contract.** Every public handler and DTO carries
//! its utoipa annotations, [`openapi::document`] assembles them, and the
//! TypeScript client is regenerated from that document in the same change
//! (PRD §24.5). Nothing on the frontend is allowed to describe this surface
//! independently.

/// The version of this crate in the running build.
///
/// Reported in the boot log so a support bundle names every component present,
/// rather than one number that only describes the binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod authentication;
pub mod error;
pub mod files;
pub mod health;
pub mod interface;
pub mod keys;
pub mod openapi;
pub mod proxy;
pub mod ratelimit;
pub mod router;
pub mod security;
pub mod setup;
pub mod state;
pub mod stream;
