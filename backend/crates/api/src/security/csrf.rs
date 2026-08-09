// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Cross-site request forgery, refused on every state-changing request.

use axum::http::{
    HeaderMap, HeaderName, Method,
    header::{HOST, ORIGIN, REFERER},
};
use subtle::ConstantTimeEq;

/// The header the SPA echoes the CSRF cookie in.
pub const CSRF_HEADER: HeaderName = HeaderName::from_static("x-afisharr-csrf");

/// What the CSRF check decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsrfDecision {
    /// The request may proceed.
    Allowed,
    /// The request declares an origin that is not this instance.
    ForeignOrigin,
    /// The echoed token is absent or does not match the cookie.
    TokenMismatch,
}

/// Judges one request.
///
/// Two independent checks, because each covers what the other misses. The
/// origin check catches a form posted from another site, and costs nothing;
/// the double-submit token catches the case where a browser sends no `Origin`
/// at all, which older clients do. Neither is a toggle, and there is no
/// configuration that turns either off (D-002-class, PRD §21.4.2).
///
/// A request carrying no session cookie is not forgeable: forgery works by
/// making a browser attach a credential it already holds, and a caller
/// authenticating with an API key in a header attaches nothing by ambient
/// authority. Those requests are allowed through, which is what keeps a
/// scripted caller from needing a browser's cookie jar.
#[must_use]
pub fn judge_csrf(
    method: &Method,
    headers: &HeaderMap,
    cookie_token: Option<&str>,
    carries_session_cookie: bool,
) -> CsrfDecision {
    if is_safe(method) || !carries_session_cookie {
        return CsrfDecision::Allowed;
    }

    if let Some(declared) = declared_origin(headers) {
        let host = headers
            .get(HOST)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !origin_matches_host(&declared, host) {
            return CsrfDecision::ForeignOrigin;
        }
    }

    let echoed = headers
        .get(&CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    match cookie_token {
        Some(cookie) if !cookie.is_empty() && constant_time_equal(cookie, echoed) => {
            CsrfDecision::Allowed
        }
        _ => CsrfDecision::TokenMismatch,
    }
}

/// Whether the method is one that never changes state.
fn is_safe(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// The host-and-port the request says it came from.
///
/// `Origin` first, `Referer` as the fallback: `Origin` is the one designed for
/// this and is sent on every cross-origin state-changing request, and `Referer`
/// is what remains when a privacy setting has stripped it down.
fn declared_origin(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get(ORIGIN)
        .or_else(|| headers.get(REFERER))?
        .to_str()
        .ok()?;
    if raw == "null" {
        // An opaque origin — a sandboxed frame or a `data:` document. It is
        // not this instance, and treating it as absent would skip the check.
        return Some(String::from("null"));
    }
    let without_scheme = raw.split_once("://").map(|(_, rest)| rest)?;
    Some(
        without_scheme
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default()
            .to_owned(),
    )
}

fn origin_matches_host(declared: &str, host: &str) -> bool {
    !declared.is_empty() && !host.is_empty() && declared.eq_ignore_ascii_case(host)
}

fn constant_time_equal(left: &str, right: &str) -> bool {
    left.len() == right.len() && left.as_bytes().ct_eq(right.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(entries: &[(HeaderName, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in entries {
            map.insert(name, HeaderValue::from_str(value).expect("a valid header"));
        }
        map
    }

    #[test]
    fn a_read_is_never_a_forgery() {
        for method in [Method::GET, Method::HEAD, Method::OPTIONS] {
            assert_eq!(
                judge_csrf(&method, &HeaderMap::new(), None, true),
                CsrfDecision::Allowed
            );
        }
    }

    #[test]
    fn a_write_with_a_matching_token_and_a_matching_origin_is_allowed() {
        let map = headers(&[
            (ORIGIN, "https://afisharr.example"),
            (HOST, "afisharr.example"),
            (CSRF_HEADER, "token-value"),
        ]);
        assert_eq!(
            judge_csrf(&Method::POST, &map, Some("token-value"), true),
            CsrfDecision::Allowed
        );
    }

    #[test]
    fn a_write_from_another_origin_is_refused_even_with_the_right_token() {
        let map = headers(&[
            (ORIGIN, "https://evil.example"),
            (HOST, "afisharr.example"),
            (CSRF_HEADER, "token-value"),
        ]);
        assert_eq!(
            judge_csrf(&Method::POST, &map, Some("token-value"), true),
            CsrfDecision::ForeignOrigin
        );
    }

    #[test]
    fn a_write_without_the_echoed_token_is_refused() {
        let map = headers(&[
            (ORIGIN, "https://afisharr.example"),
            (HOST, "afisharr.example"),
        ]);
        assert_eq!(
            judge_csrf(&Method::POST, &map, Some("token-value"), true),
            CsrfDecision::TokenMismatch
        );
    }

    #[test]
    fn a_write_with_no_cookie_at_all_is_refused_rather_than_waved_through() {
        let map = headers(&[
            (ORIGIN, "https://afisharr.example"),
            (HOST, "afisharr.example"),
            (CSRF_HEADER, "token-value"),
        ]);
        assert_eq!(
            judge_csrf(&Method::POST, &map, None, true),
            CsrfDecision::TokenMismatch
        );
    }

    #[test]
    fn an_opaque_origin_is_refused() {
        let map = headers(&[
            (ORIGIN, "null"),
            (HOST, "afisharr.example"),
            (CSRF_HEADER, "token-value"),
        ]);
        assert_eq!(
            judge_csrf(&Method::POST, &map, Some("token-value"), true),
            CsrfDecision::ForeignOrigin
        );
    }

    #[test]
    fn the_referer_stands_in_when_the_origin_was_stripped() {
        let map = headers(&[
            (REFERER, "https://evil.example/page"),
            (HOST, "afisharr.example"),
            (CSRF_HEADER, "token-value"),
        ]);
        assert_eq!(
            judge_csrf(&Method::POST, &map, Some("token-value"), true),
            CsrfDecision::ForeignOrigin
        );
    }

    #[test]
    fn a_caller_with_no_session_cookie_is_not_forgeable_and_is_allowed() {
        // An API-key caller attaches no ambient credential, so there is nothing
        // for another site to make a browser send on its behalf.
        assert_eq!(
            judge_csrf(&Method::POST, &HeaderMap::new(), None, false),
            CsrfDecision::Allowed
        );
    }

    #[test]
    fn a_write_with_no_origin_header_still_needs_the_token() {
        let map = headers(&[(HOST, "afisharr.example"), (CSRF_HEADER, "token-value")]);
        assert_eq!(
            judge_csrf(&Method::POST, &map, Some("token-value"), true),
            CsrfDecision::Allowed
        );
        assert_eq!(
            judge_csrf(&Method::POST, &map, Some("other"), true),
            CsrfDecision::TokenMismatch
        );
    }
}
