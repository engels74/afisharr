// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The limit table, and the counters that enforce it.
//!
//! Keyed through [`crate::proxy::ClientContext`], never through a header a
//! caller can set. That is what makes the limits real rather than decorative
//! (`I-SEC-1`, D-029).

mod counter;
mod counters;
mod limiter;
mod policy;

pub use counter::Decision;
pub use limiter::RateLimiter;
pub use policy::{Bucket, Policy};

/// The one refusal a spent request budget produces.
///
/// Stated once because two callers produce it — the layer that counts
/// credential-less traffic and the extractor that counts a refused credential —
/// and a caller cannot be told two different things about one limit (P7).
#[must_use]
pub fn too_many_requests(retry_after_seconds: u64) -> crate::error::AppError {
    crate::error::AppError::new(
        crate::error::Problem::new(
            crate::error::ErrorCode::RateLimited,
            "Too many requests. Try again shortly.",
        )
        .retry_after(retry_after_seconds),
    )
}
