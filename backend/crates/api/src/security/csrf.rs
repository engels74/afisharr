// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Cross-site request forgery, refused on every state-changing request.

use axum::http::{HeaderMap, HeaderName, Method};
use subtle::ConstantTimeEq;

use crate::{
    proxy::PublicOrigin,
    security::declared::{declared_origin, declares_this_instance},
};

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
/// A request carrying no ambient credential is not forgeable: forgery works by
/// making a browser attach a credential it already holds, and a caller
/// authenticating with an API key in a header attaches nothing by ambient
/// authority. Those requests are allowed through, which is what keeps a
/// scripted caller from needing a browser's cookie jar.
///
/// The session cookie is not the only ambient credential this surface has. The
/// setup claim is one too: it is a cookie, the browser attaches it to any
/// request another origin can cause, and behind it sit the routes that create
/// the administrator and finish setup. A hostile page on the same registrable
/// domain — another container on the same host, a second service behind the
/// same name — can post to `/api/setup/complete` without ever reading the
/// answer. So `carries_ambient_credential` is every cookie that authorises
/// something, and the caller is the one that enumerates them (PRD §21.4.2).
///
/// `configured` is this instance's `publicOrigin`, and it is what the declared
/// origin is compared against whenever the operator has set one. The fallback
/// — the request's own `Host` — is what a first-run instance has and nothing
/// more: `Host` is written by whoever is calling, and every mainstream reverse
/// proxy rewrites it. nginx's `proxy_pass` alone sets `Host` to the upstream's
/// name, so an instance judging against it sees `Host: afisharr:8484` beside
/// `Origin: https://media.example` and refuses every write the operator makes,
/// with a message about cross-site attacks that are not happening.
#[must_use]
pub fn judge_csrf(
    method: &Method,
    headers: &HeaderMap,
    cookie_token: Option<&str>,
    carries_ambient_credential: bool,
    configured: Option<&PublicOrigin>,
) -> CsrfDecision {
    if is_safe(method) || !carries_ambient_credential {
        return CsrfDecision::Allowed;
    }

    if let Some(declared) = declared_origin(headers)
        && !declares_this_instance(&declared, headers, configured)
    {
        return CsrfDecision::ForeignOrigin;
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

fn constant_time_equal(left: &str, right: &str) -> bool {
    left.len() == right.len() && left.as_bytes().ct_eq(right.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use axum::http::{
        HeaderValue,
        header::{HOST, ORIGIN, REFERER},
    };

    use super::*;

    fn headers(entries: &[(HeaderName, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in entries {
            map.insert(name, HeaderValue::from_str(value).expect("a valid header"));
        }
        map
    }

    /// An instance with no `publicOrigin` set, which is the default.
    fn unconfigured(
        map: &HeaderMap,
        cookie_token: Option<&str>,
        carries_ambient_credential: bool,
    ) -> CsrfDecision {
        judge_csrf(
            &Method::POST,
            map,
            cookie_token,
            carries_ambient_credential,
            None,
        )
    }

    fn configured(value: &str) -> PublicOrigin {
        PublicOrigin::parse(value).expect("a valid origin")
    }

    #[test]
    fn a_read_is_never_a_forgery() {
        for method in [Method::GET, Method::HEAD, Method::OPTIONS] {
            assert_eq!(
                judge_csrf(&method, &HeaderMap::new(), None, true, None),
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
            unconfigured(&map, Some("token-value"), true),
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
            unconfigured(&map, Some("token-value"), true),
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
            unconfigured(&map, Some("token-value"), true),
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
        assert_eq!(unconfigured(&map, None, true), CsrfDecision::TokenMismatch);
    }

    #[test]
    fn an_opaque_origin_is_refused() {
        let map = headers(&[
            (ORIGIN, "null"),
            (HOST, "afisharr.example"),
            (CSRF_HEADER, "token-value"),
        ]);
        assert_eq!(
            unconfigured(&map, Some("token-value"), true),
            CsrfDecision::ForeignOrigin
        );
        // And against a configured origin too: `null` is not a URL, so it
        // covers nothing.
        assert_eq!(
            judge_csrf(
                &Method::POST,
                &map,
                Some("token-value"),
                true,
                Some(&configured("https://afisharr.example")),
            ),
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
            unconfigured(&map, Some("token-value"), true),
            CsrfDecision::ForeignOrigin
        );
    }

    #[test]
    fn a_caller_with_no_ambient_credential_is_not_forgeable_and_is_allowed() {
        // An API-key caller attaches no ambient credential, so there is nothing
        // for another site to make a browser send on its behalf.
        assert_eq!(
            unconfigured(&HeaderMap::new(), None, false),
            CsrfDecision::Allowed
        );
    }

    #[test]
    fn a_setup_write_carrying_only_the_claim_cookie_is_still_judged() {
        // The forgery this closes: a hostile same-site origin posts to
        // `/api/setup/complete`, the browser attaches the claim cookie, and the
        // attacker never has to read the answer.
        let map = headers(&[
            (ORIGIN, "https://evil.example"),
            (HOST, "afisharr.example"),
            (CSRF_HEADER, "token-value"),
        ]);
        assert_eq!(
            unconfigured(&map, Some("token-value"), true),
            CsrfDecision::ForeignOrigin
        );
        assert_eq!(
            unconfigured(&HeaderMap::new(), None, true),
            CsrfDecision::TokenMismatch
        );
    }

    #[test]
    fn a_write_with_no_origin_header_still_needs_the_token() {
        let map = headers(&[(HOST, "afisharr.example"), (CSRF_HEADER, "token-value")]);
        assert_eq!(
            unconfigured(&map, Some("token-value"), true),
            CsrfDecision::Allowed
        );
        assert_eq!(
            unconfigured(&map, Some("other"), true),
            CsrfDecision::TokenMismatch
        );
    }

    #[test]
    fn a_proxy_that_rewrites_host_does_not_refuse_every_write() {
        // The deployment this closes, and it is the ordinary one: nginx's
        // `proxy_pass` sets `Host` to the upstream's own name, so the instance
        // sees `Host: afisharr:8484` while the browser declares the address the
        // operator actually reached it at. Judged against `Host`, every write
        // an operator makes is refused as a cross-site attack — sign-in, the
        // setup claim, and creating the administrator included.
        let map = headers(&[
            (ORIGIN, "https://media.example"),
            (HOST, "afisharr:8484"),
            (CSRF_HEADER, "token-value"),
        ]);
        assert_eq!(
            unconfigured(&map, Some("token-value"), true),
            CsrfDecision::ForeignOrigin,
            "the Host fallback is what the configured origin replaces"
        );
        assert_eq!(
            judge_csrf(
                &Method::POST,
                &map,
                Some("token-value"),
                true,
                Some(&configured("https://media.example")),
            ),
            CsrfDecision::Allowed
        );
    }
}
