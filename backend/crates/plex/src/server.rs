// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The connection to one Plex Media Server.
//!
//! Everything the rest of this crate calls goes out through
//! [`PlexServerClient`]: it owns the address, the token, the `X-Plex-*` header
//! set, and the one envelope every Plex answer arrives in. A module that built
//! its own request would be a second place the token could be forgotten, and a
//! second place a deadline could be omitted (PRD §21.2.5, P7).
//!
//! [`MachineIdentifier`] is the identity the whole binding rests on. It is
//! fetched by its own call, because `I-ID-5` has to be checkable without
//! reading a library: a server swap must be detectable in one cheap request at
//! the head of every pass, not discovered part-way through one.

mod address;
mod binding;
mod client;
mod container;
mod error;
mod machine;
mod token;

pub use address::{AddressError, ServerAddress, redact_credentials};
pub use binding::{BindingVerdict, verify_binding};
pub use client::PlexServerClient;
pub use error::ServerError;
pub use machine::{MachineIdentifier, ServerIdentity};
pub use token::ServerToken;

pub(crate) use container::Container;
