// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Which plex.tv account a token belongs to.
//!
//! A completed PIN exchange proves the caller holds *a* plex.tv account. It
//! does not say whose. Signing someone in on the strength of the exchange
//! alone would let anyone with a plex.tv account walk into an instance that
//! offers Plex sign-in — so the token is turned into an account identity
//! before it is worth anything.

mod lookup;

pub(crate) use lookup::AccountBody;
pub use lookup::PlexAccount;
