// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Minting a session and taking one away.

use afisharr_core::{
    entropy,
    sessions::{
        ABSOLUTE_LIFETIME_MILLIS, CreateSession, RevokeAllForUser, RevokeSession, SessionToken,
    },
};
use axum_extra::extract::cookie::Cookie;

use crate::{
    error::{AppError, AppResult},
    proxy::{ClientContext, Scheme},
    security::{CSRF_COOKIE, SESSION_COOKIE, expire, set},
    state::ApiState,
};

/// A session and the two cookies that carry it.
///
/// The token itself is consumed building the cookie and is not exposed: the
/// only copies that exist afterwards are the browser's and the digest in the
/// table.
#[derive(Debug)]
pub struct IssuedSession {
    /// The `Set-Cookie` values to attach, in order.
    pub cookies: Vec<Cookie<'static>>,
}

/// Creates a session for `user_id` and returns the cookies that carry it.
///
/// Two cookies, not one. The session cookie is `HttpOnly` so no script can
/// read it; the CSRF cookie is deliberately readable, because the double-submit
/// check needs the page to echo it and a value the page cannot read is a value
/// it cannot echo. Neither authenticates on its own.
///
/// # Errors
/// Returns [`AppError`] when the session could not be written.
pub async fn issue(
    state: &ApiState,
    user_id: &str,
    client: ClientContext,
    user_agent: Option<String>,
) -> AppResult<IssuedSession> {
    let token = SessionToken::generate();
    let csrf = hex_token();

    state
        .database()
        .writer()
        .submit(CreateSession {
            digest: token.digest().to_owned(),
            user_id: user_id.to_owned(),
            user_agent,
            ip: Some(client.address.to_string()),
            at: state.clock().now(),
        })
        .await
        .map_err(AppError::internal)?;

    Ok(IssuedSession {
        cookies: vec![
            set(
                SESSION_COOKIE,
                token.value().to_owned(),
                "/",
                ABSOLUTE_LIFETIME_MILLIS / 1000,
                client.scheme,
                true,
            ),
            set(
                CSRF_COOKIE,
                csrf,
                "/",
                ABSOLUTE_LIFETIME_MILLIS / 1000,
                client.scheme,
                false,
            ),
        ],
    })
}

/// Revokes one session and returns the cookies that clear it.
///
/// # Errors
/// Returns [`AppError`] when the revocation could not be written.
pub async fn revoke(
    state: &ApiState,
    digest: &str,
    scheme: Scheme,
) -> AppResult<Vec<Cookie<'static>>> {
    state
        .database()
        .writer()
        .submit(RevokeSession {
            digest: digest.to_owned(),
            at: state.clock().now(),
        })
        .await
        .map_err(AppError::internal)?;

    Ok(vec![
        expire(SESSION_COOKIE, "/", scheme),
        expire(CSRF_COOKIE, "/", scheme),
    ])
}

/// Revokes every session a user holds except the one named, and reports how
/// many went.
///
/// # Errors
/// Returns [`AppError`] when the revocation could not be written.
pub(crate) async fn revoke_others(
    state: &ApiState,
    user_id: &str,
    keep: Option<String>,
) -> AppResult<u64> {
    state
        .database()
        .writer()
        .submit(RevokeAllForUser {
            user_id: user_id.to_owned(),
            keep,
            at: state.clock().now(),
        })
        .await
        .map_err(AppError::internal)
}

/// A 256-bit value, hex-encoded, for the CSRF cookie.
fn hex_token() -> String {
    hex_encode(&entropy::bytes::<32>())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_csrf_token_is_sixty_four_hex_characters() {
        let token = hex_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn two_csrf_tokens_differ() {
        assert_ne!(hex_token(), hex_token());
    }

    #[test]
    fn hex_encoding_is_lowercase_and_zero_padded() {
        assert_eq!(hex_encode(&[0x00, 0x0f, 0xff]), "000fff");
    }
}
