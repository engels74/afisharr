// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # afisharr-core
//!
//! Afisharr's domain core: the persistence spine every other crate writes
//! through, and the domain types the product is defined in.
//!
//! What lives here is deliberately I/O-free where it can be. Pure domain
//! logic takes an injected [`time::Clock`] rather than reading the wall clock,
//! so a pass is a function of its inputs and the instant it was evaluated at.

/// The version of this crate in the running build.
///
/// Reported in the boot log so a support bundle names every component present,
/// rather than one number that only describes the binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod accounts;
pub mod api_keys;
pub mod backup;
pub mod digest;
pub mod entropy;
pub mod filesystem;
pub mod identifier;
pub mod instance;
pub mod integrity;
pub mod jobs;
pub mod leases;
pub mod lifecycle;
pub mod locale;
pub mod plex_pin;
pub mod plex_server;
pub mod projection;
pub mod secrets;
pub mod sessions;
pub mod settings;
pub mod setup;
pub mod storage;
pub mod time;
