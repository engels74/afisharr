// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading an answer's body, and how much of one this instance will hold.
//!
//! Both halves of "how much" live here, because they are the same question
//! asked at two sizes: what a body may cost to read at all, and how much of a
//! refusal is worth carrying into an error the operator reads.

use crate::outbound::OutboundError;

/// How much of an upstream answer this client will hold in memory.
///
/// Every outbound call ends in one allocation of whatever the other side sends,
/// and two of the routes that reach here — starting and polling a Plex pin —
/// answer before any credential is required. Reading to completion with no cap
/// therefore let an upstream decide this container's memory: plex.tv having an
/// incident, a hijacked DNS entry, an operator's own MITM proxy, and the process
/// allocates until the OOM killer takes it, and the database and every open
/// session with it.
///
/// Eight megabytes is far above any answer these adapters read — a pin resource
/// is a few hundred bytes, an account body a few kilobytes — and far below what
/// a container with a database in it can lose.
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// Reads a response body, stopping at [`MAX_BODY_BYTES`].
///
/// Streamed rather than buffered whole, because a cap checked after the read has
/// already spent the memory it was meant to bound. `Content-Length` is checked
/// first where the upstream declares one, so the common case is refused before a
/// byte of body is read; a chunked answer that declares nothing is bounded by
/// the running total instead, which is the case a hostile upstream would use.
///
/// # Errors
/// Returns [`OutboundError::Oversized`] when the body passes the cap, and
/// [`OutboundError::Unreachable`] when it stops arriving part-way.
pub(super) async fn within_cap(
    mut response: reqwest::Response,
    host: &str,
    timeout_millis: u64,
) -> Result<String, OutboundError> {
    let oversized = || OutboundError::Oversized {
        host: host.to_owned(),
        limit_bytes: MAX_BODY_BYTES,
    };
    if response
        .content_length()
        .is_some_and(|declared| declared > MAX_BODY_BYTES as u64)
    {
        return Err(oversized());
    }

    let mut collected: Vec<u8> = Vec::new();
    loop {
        // Headers are not an answer. A connection that drops or times out
        // while the body streams has told us nothing, and turning that into an
        // empty body would hand the adapter a "malformed response" it cannot
        // act on while discarding the transport failure that actually
        // happened — the answered-versus-unreachable distinction `I-SRC-1` is
        // built on, collapsed exactly where it matters (P1).
        let chunk = response
            .chunk()
            .await
            .map_err(|source| OutboundError::Unreachable {
                host: host.to_owned(),
                timeout_millis,
                source,
            })?;
        let Some(chunk) = chunk else {
            break;
        };
        if collected.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
            return Err(oversized());
        }
        collected.extend_from_slice(&chunk);
    }

    // Lossy rather than refused: these adapters read JSON, which is UTF-8 by
    // definition, and a body with one bad byte in it is a body the parser should
    // get to reject in its own terms rather than one this layer renames.
    Ok(String::from_utf8_lossy(&collected).into_owned())
}

/// How much of a refusal body is worth keeping for the collapsed detail.
const BODY_EXCERPT_BYTES: usize = 512;

/// The head of a refusal body, cut on a character boundary.
pub(super) fn truncated(body: &str) -> String {
    if body.len() <= BODY_EXCERPT_BYTES {
        return body.to_owned();
    }
    let mut end = BODY_EXCERPT_BYTES;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    body[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_at_the_excerpt_boundary_is_kept_whole() {
        let body = "a".repeat(BODY_EXCERPT_BYTES);
        assert_eq!(truncated(&body).len(), BODY_EXCERPT_BYTES);
    }

    #[test]
    fn a_long_body_is_cut_on_a_character_boundary() {
        let body = "é".repeat(BODY_EXCERPT_BYTES);
        let cut = truncated(&body);
        assert!(cut.len() <= BODY_EXCERPT_BYTES);
        assert!(body.starts_with(&cut));
    }
}
