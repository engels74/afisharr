// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The fixed response-header set from PRD §21.4.4.

use axum::http::{
    HeaderMap, HeaderName, HeaderValue,
    header::{
        CONTENT_SECURITY_POLICY, REFERRER_POLICY, STRICT_TRANSPORT_SECURITY, X_CONTENT_TYPE_OPTIONS,
    },
};

use crate::proxy::Scheme;

/// `Permissions-Policy` has no constant in the `http` crate.
const PERMISSIONS_POLICY: HeaderName = HeaderName::from_static("permissions-policy");

/// One year, with subdomains. Emitted only over HTTPS.
const HSTS: HeaderValue = HeaderValue::from_static("max-age=31536000; includeSubDomains");

const NOSNIFF: HeaderValue = HeaderValue::from_static("nosniff");
const NO_REFERRER: HeaderValue = HeaderValue::from_static("no-referrer");

/// Camera, microphone, and geolocation denied to every origin.
const PERMISSIONS: HeaderValue =
    HeaderValue::from_static("camera=(), microphone=(), geolocation=()");

/// The policy that governs what the served page may load and run.
///
/// `'unsafe-inline'` never appears in `script-src`. The SPA's one inline
/// bootstrap is admitted by the SHA-256 of its own bytes, computed at boot from
/// the document the binary is about to serve — so the policy is exactly as
/// tight as a nonce and does not need a server runtime to mint one. A change to
/// the SPA changes the hash with it; nothing has to be kept in step by hand.
///
/// `style-src` allows inline styles. Svelte emits element-level `style`
/// attributes for transitions, and hashing those is not possible; the
/// alternative is banning a framework feature to close a hole that requires
/// script execution to exploit, which script-src already denies.
#[derive(Debug, Clone)]
pub struct ContentSecurityPolicy(HeaderValue);

impl ContentSecurityPolicy {
    /// Builds the policy, admitting the given inline-script digests.
    ///
    /// Each digest is the base64 SHA-256 of one inline script's exact bytes,
    /// written the way CSP expects: `sha256-<base64>`.
    #[must_use]
    pub fn with_script_digests(digests: &[String]) -> Self {
        let mut script_src = String::from("script-src 'self'");
        for digest in digests {
            script_src.push_str(" '");
            script_src.push_str(digest);
            script_src.push('\'');
        }

        let policy = [
            "default-src 'self'",
            script_src.as_str(),
            "style-src 'self' 'unsafe-inline'",
            "img-src 'self' data: blob:",
            "font-src 'self' data:",
            // The SPA talks to this origin and nothing else: Afisharr collects
            // nothing and reaches no third party from the browser (D-038).
            "connect-src 'self'",
            "frame-ancestors 'none'",
            "base-uri 'self'",
            "form-action 'self'",
            "object-src 'none'",
        ]
        .join("; ");

        Self(
            HeaderValue::from_str(&policy)
                .unwrap_or_else(|_| HeaderValue::from_static("default-src 'self'")),
        )
    }

    /// The header value.
    #[must_use]
    pub fn value(&self) -> &HeaderValue {
        &self.0
    }
}

impl Default for ContentSecurityPolicy {
    fn default() -> Self {
        Self::with_script_digests(&[])
    }
}

/// Writes the whole header set onto one response.
///
/// `insert` rather than `append` throughout: a handler that already set one of
/// these would otherwise produce two values for a header whose meaning is the
/// strictest of them, and "strictest of two" is not a property anyone should
/// have to reason about per route.
pub fn apply_security_headers(
    headers: &mut HeaderMap,
    policy: &ContentSecurityPolicy,
    scheme: Scheme,
) {
    headers.insert(CONTENT_SECURITY_POLICY, policy.value().clone());
    headers.insert(X_CONTENT_TYPE_OPTIONS, NOSNIFF);
    headers.insert(REFERRER_POLICY, NO_REFERRER);
    headers.insert(PERMISSIONS_POLICY, PERMISSIONS);

    // Only over HTTPS, and only when a trusted proxy vouched for it. Sending
    // HSTS over plaintext asks a browser to refuse the only scheme the operator
    // can currently reach the instance on.
    if scheme.is_secure() {
        headers.insert(STRICT_TRANSPORT_SECURITY, HSTS);
    } else {
        headers.remove(STRICT_TRANSPORT_SECURITY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applied(scheme: Scheme) -> HeaderMap {
        let mut headers = HeaderMap::new();
        apply_security_headers(&mut headers, &ContentSecurityPolicy::default(), scheme);
        headers
    }

    #[test]
    fn every_response_carries_the_four_unconditional_headers() {
        let headers = applied(Scheme::Http);
        for name in [
            CONTENT_SECURITY_POLICY,
            X_CONTENT_TYPE_OPTIONS,
            REFERRER_POLICY,
            PERMISSIONS_POLICY,
        ] {
            assert!(headers.contains_key(&name), "{name} is missing");
        }
    }

    #[test]
    fn hsts_is_emitted_over_https_and_withheld_over_plaintext() {
        assert!(
            applied(Scheme::Https)
                .get(STRICT_TRANSPORT_SECURITY)
                .is_some()
        );
        assert!(
            applied(Scheme::Http)
                .get(STRICT_TRANSPORT_SECURITY)
                .is_none()
        );
    }

    #[test]
    fn the_policy_denies_framing_and_never_allows_inline_script() {
        let policy = ContentSecurityPolicy::default();
        let text = policy.value().to_str().expect("the policy is ASCII");
        assert!(text.contains("frame-ancestors 'none'"), "{text}");
        assert!(text.contains("default-src 'self'"), "{text}");
        assert!(
            !text.contains("script-src 'self' 'unsafe-inline'"),
            "{text}"
        );
    }

    #[test]
    fn a_script_digest_is_admitted_by_hash_rather_than_by_unsafe_inline() {
        let policy = ContentSecurityPolicy::with_script_digests(&[
            "sha256-abc123".to_owned(),
            "sha256-def456".to_owned(),
        ]);
        let text = policy.value().to_str().expect("the policy is ASCII");
        assert!(
            text.contains("script-src 'self' 'sha256-abc123' 'sha256-def456'"),
            "{text}"
        );
        let script_src = text
            .split("; ")
            .find(|directive| directive.starts_with("script-src"))
            .expect("script-src is present");
        assert!(!script_src.contains("unsafe-inline"), "{script_src}");
    }

    #[test]
    fn applying_twice_leaves_one_value_per_header() {
        let mut headers = HeaderMap::new();
        let policy = ContentSecurityPolicy::default();
        apply_security_headers(&mut headers, &policy, Scheme::Https);
        apply_security_headers(&mut headers, &policy, Scheme::Https);
        assert_eq!(
            headers.get_all(CONTENT_SECURITY_POLICY).iter().count(),
            1,
            "a repeated apply must not stack values"
        );
    }

    #[test]
    fn a_response_that_was_https_and_is_replayed_as_http_loses_hsts() {
        let mut headers = HeaderMap::new();
        let policy = ContentSecurityPolicy::default();
        apply_security_headers(&mut headers, &policy, Scheme::Https);
        apply_security_headers(&mut headers, &policy, Scheme::Http);
        assert!(headers.get(STRICT_TRANSPORT_SECURITY).is_none());
    }
}
