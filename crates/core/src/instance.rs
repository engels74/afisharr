// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Who this installation is.
//!
//! The single `instance` row carries the identity that must survive every
//! restart — above all `client_identifier`, which plex.tv binds tokens to and
//! which is generated once and never regenerated (PRD §19.5).

mod identity;

pub use identity::{EnsureInstance, Instance, NewInstance, load};
