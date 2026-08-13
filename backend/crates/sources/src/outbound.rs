// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The one instrumented outbound client (PRD §21.2.5).
//!
//! A stalled connection never raises an error, so a retry policy waiting for
//! an exception waits forever. The deadline is therefore a property of the
//! client, not an argument at the call site: a caller may shorten it and
//! cannot omit it.

mod body;
mod client;
mod deadline;
mod error;

pub use client::{OutboundClient, Response};
pub use deadline::Deadline;
pub use error::OutboundError;
// The transport's own types, re-exported so an adapter names the seam rather
// than the crate behind it. An adapter that depends on `reqwest` directly is an
// adapter that can build its own client (PRD §21.2.5).
pub use reqwest::{
    Method,
    header::{HeaderName, HeaderValue},
};
