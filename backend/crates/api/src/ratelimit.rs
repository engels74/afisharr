// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The limit table, and the counters that enforce it.
//!
//! Keyed through [`crate::proxy::ClientContext`], never through a header a
//! caller can set. That is what makes the limits real rather than decorative
//! (`I-SEC-1`, D-029).

mod limiter;
mod policy;

pub use limiter::{Decision, RateLimiter};
pub use policy::{Bucket, Policy};
