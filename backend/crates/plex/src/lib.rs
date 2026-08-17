// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # afisharr-plex
//!
//! The Plex client: the protocol surface Afisharr actually calls, and no more.
//!
//! [`pin`] and [`account`] are the plex.tv half: the PIN and OAuth token
//! exchange behind the login flow. Everything else here is the server half —
//! [`server`] owns the connection and the machine identifier `I-ID-5` rests on,
//! and [`libraries`], [`collections`], [`hubs`], [`labels`], [`streams`],
//! [`artwork`], and [`discovery`] are the calls Afisharr actually makes,
//! and no more.
//!
//! The adversarial fake (D-036) lives in `fake`, behind the `fake` feature so
//! it is compiled for tests and absent from the shipped binary.
//!
//! Every request leaves through the one instrumented outbound client in
//! `afisharr-sources` (PRD §21.2.5). This crate owns the protocol; it does not
//! own a transport.

/// The version of this crate in the running build.
///
/// Reported in the boot log so a support bundle names every component present,
/// rather than one number that only describes the binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod account;
pub mod artwork;
pub mod collections;
pub mod discovery;
pub mod edits;
#[cfg(feature = "fake")]
pub mod fake;
pub mod hubs;
pub mod identity;
pub mod labels;
pub mod libraries;
pub mod pin;
pub mod server;
pub mod streams;
mod wire;
