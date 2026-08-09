// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! How Afisharr identifies itself to Plex.
//!
//! plex.tv binds every token it issues to the `X-Plex-Client-Identifier` that
//! asked for it. Get the header wrong on a later request and the token is
//! refused for a reason nothing in the response explains, so the identity is a
//! value the caller constructs once and passes, never a set of strings
//! assembled at each call site (P7).

mod headers;

pub use headers::{ClientIdentity, IdentityError, PLEX_CLIENT_IDENTIFIER, PLEX_TOKEN};
