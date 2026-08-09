// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The plex.tv PIN and OAuth token exchange.
//!
//! Both variants are one flow: create a pin resource, show the operator
//! something (a four-character code, or a URL to sign in at), and poll the same
//! pin until a token appears or the pin expires. Only the middle step differs,
//! which is why they share the polling machinery rather than having one each
//! (P7).

mod authorization;
mod client;
mod error;
mod resource;

pub use authorization::{AuthorizationUrl, Mode};
pub use client::PlexTvClient;
pub use error::PinError;
pub use resource::{PinPoll, PinResource};
