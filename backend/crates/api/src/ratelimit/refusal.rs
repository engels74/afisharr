// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a spent budget answers, stated once.
//!
//! Apart from the counters, because it is the only part of the limit a caller
//! ever sees. [`RateLimiter::record`] reports a
//! [`Decision`], and every handler that acted on one used to write the same
//! `match` around the same `Problem` — six of them, two sharing a name and
//! differing in their body, so a change to how a refusal is rendered reached
//! one of six answers. The shape lives here and the sentence stays with the
//! caller, because a spent plex.tv allowance is not a spent sign-in allowance
//! and an operator told the wrong one goes looking in the wrong place (P7).

use std::net::IpAddr;

use crate::{
    error::{AppError, ErrorCode, Problem},
    ratelimit::{Bucket, Decision, RateLimiter},
};

impl RateLimiter {
    /// Takes one attempt from `bucket`, or refuses it in `message`'s terms.
    ///
    /// The only way a handler consults a limit. There was a second — one that
    /// asked whether the bucket was spent without spending it, for the sign-in
    /// path where a failure is only known after the password hash — and it is
    /// gone, because a question that changes nothing bounds nothing: a burst
    /// arriving inside one instant was answered "not spent" all the way down.
    /// A caller takes its attempt first and calls [`RateLimiter::forget`] when
    /// the attempt turns out to have been the operator's own.
    ///
    /// # Errors
    /// Returns the `rateLimited` refusal, carrying the retry time, when the
    /// bucket's allowance is spent.
    pub fn spend(
        &self,
        bucket: &Bucket,
        address: Option<IpAddr>,
        message: &str,
    ) -> Result<(), AppError> {
        refuse(self.record(bucket, address), message)
    }
}

/// Turns a [`Decision`] into the refusal a handler returns.
fn refuse(decision: Decision, message: &str) -> Result<(), AppError> {
    match decision {
        Decision::Allowed => Ok(()),
        Decision::Refused {
            retry_after_seconds,
        } => Err(refused(retry_after_seconds, message)),
    }
}

/// The refusal a spent budget produces, in one bucket's own terms.
///
/// The one place the shape is built. Everything that meters a request reaches
/// it through [`RateLimiter::spend`]; this
/// stays public for the callers that hold a [`Decision`] already.
#[must_use]
pub fn refused(retry_after_seconds: u64, message: impl Into<String>) -> AppError {
    AppError::new(Problem::new(ErrorCode::RateLimited, message).retry_after(retry_after_seconds))
}

/// The refusal for the buckets that meter requests rather than attempts.
///
/// Stated once because two callers produce it — the layer that counts
/// credential-less traffic and the extractor that counts a refused credential —
/// and a caller cannot be told two different things about one limit (P7).
#[must_use]
pub fn too_many_requests(retry_after_seconds: u64) -> AppError {
    refused(retry_after_seconds, "Too many requests. Try again shortly.")
}
