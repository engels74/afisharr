// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The fixed response-header set from PRD §21.4.4.

use axum::http::{
    HeaderMap, HeaderName, HeaderValue,
    header::{
        CONTENT_SECURITY_POLICY, REFERRER_POLICY, STRICT_TRANSPORT_SECURITY, X_CONTENT_TYPE_OPTIONS,
    },
};

use crate::proxy::{ClientContext, PublicOrigin};

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
///
/// `configured` is this instance's `publicOrigin`, and it is the second of the
/// two things HSTS needs. See [`emits_hsts`] for why one is not enough.
pub fn apply_security_headers(
    headers: &mut HeaderMap,
    policy: &ContentSecurityPolicy,
    client: ClientContext,
    configured: Option<&PublicOrigin>,
) {
    headers.insert(CONTENT_SECURITY_POLICY, policy.value().clone());
    headers.insert(X_CONTENT_TYPE_OPTIONS, NOSNIFF);
    headers.insert(REFERRER_POLICY, NO_REFERRER);
    headers.insert(PERMISSIONS_POLICY, PERMISSIONS);

    if emits_hsts(client, configured) {
        headers.insert(STRICT_TRANSPORT_SECURITY, HSTS);
    } else {
        headers.remove(STRICT_TRANSPORT_SECURITY);
    }
}

/// Whether this answer may carry `Strict-Transport-Security`.
///
/// Two conditions, and the second is not redundant. The resolved scheme is
/// derived from `X-Forwarded-Proto`, and that header is only as trustworthy as
/// the chain in front of it: a proxy that sets `X-Forwarded-For` without also
/// setting `X-Forwarded-Proto` passes the client's own claim straight through,
/// and nothing in the header can tell the two apart. A forged `https` then
/// pins the whole registrable domain — every unrelated subdomain included — to
/// HTTPS in that browser for a year, with no way for the operator to click
/// through it.
///
/// So the durable half rests on the one statement no caller can write: the
/// operator having configured `publicOrigin` as an `https` address. A forged
/// header can still make a single answer look secure; it cannot make this
/// instance ask a browser to remember it.
///
/// The scheme condition stays because it is the reverse mistake: sending HSTS
/// over plaintext asks a browser to refuse the only scheme an operator on the
/// LAN can currently reach the instance on.
///
/// There is a third condition, and it is the same argument applied to the
/// answer `publicOrigin` itself produces. `ClientContext::at_configured_origin`
/// reads a forwarded request as HTTPS when it arrived at the `https` address
/// the operator declared *and no hop said anything about the scheme* — which is
/// a real TLS proxy that omitted `X-Forwarded-Proto` and a proxy still
/// listening on `:80`, wearing the same headers. Nothing can separate them, so
/// the durable half is withheld from both: an inferred reading sets `Secure` on
/// the cookie, which a plaintext browser discards and which comes back when the
/// deployment is corrected, and never asks that browser to remember the name
/// for a year.
fn emits_hsts(client: ClientContext, configured: Option<&PublicOrigin>) -> bool {
    client.scheme.is_secure()
        && !client.scheme_inferred
        && configured.is_some_and(PublicOrigin::is_secure)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::proxy::Scheme;

    fn secure_origin() -> PublicOrigin {
        PublicOrigin::parse("https://afisharr.example").expect("a valid origin")
    }

    /// A request the chain itself said arrived over `scheme`.
    fn stated(scheme: Scheme) -> ClientContext {
        ClientContext {
            address: "203.0.113.9".parse().expect("a valid address"),
            scheme,
            scheme_inferred: false,
            forwarded_hops: 1,
        }
    }

    /// A request read as HTTPS from `publicOrigin` alone.
    fn inferred() -> ClientContext {
        ClientContext {
            scheme_inferred: true,
            ..stated(Scheme::Https)
        }
    }

    /// The header set for a deployment whose `publicOrigin` names HTTPS.
    fn applied(scheme: Scheme) -> HeaderMap {
        applied_for(stated(scheme), Some(&secure_origin()))
    }

    fn applied_for(client: ClientContext, configured: Option<&PublicOrigin>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        apply_security_headers(
            &mut headers,
            &ContentSecurityPolicy::default(),
            client,
            configured,
        );
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
    fn a_forged_scheme_alone_never_pins_a_domain_for_a_year() {
        // The harm this closes. A proxy that sets `X-Forwarded-For` and not
        // `X-Forwarded-Proto` passes the client's own claim through, and the
        // header cannot say which of the two wrote it. Resting HSTS on that
        // alone lets a caller pin the whole registrable domain in the
        // operator's browser for a year, with no way to click through it.
        assert!(
            applied_for(stated(Scheme::Https), None)
                .get(STRICT_TRANSPORT_SECURITY)
                .is_none(),
            "an unconfigured instance must not emit HSTS on a claimed scheme"
        );

        let plaintext = PublicOrigin::parse("http://192.168.1.10:8484").expect("a valid origin");
        assert!(
            applied_for(stated(Scheme::Https), Some(&plaintext))
                .get(STRICT_TRANSPORT_SECURITY)
                .is_none(),
            "an operator who configured a plaintext origin has not asked for HSTS"
        );
    }

    #[test]
    fn a_scheme_inferred_from_the_configured_origin_pins_nothing() {
        // The deployment this closes: a proxy listening on `:80` that forwards
        // `X-Forwarded-For` and sets no `X-Forwarded-Proto`, at the `https`
        // name the operator configured. It is indistinguishable from a TLS
        // proxy that omitted the header, so the reading stands for the cookie
        // flag — discarded by the browser, and back the moment the proxy is
        // corrected — and never for the year the browser would remember.
        assert!(
            applied_for(inferred(), Some(&secure_origin()))
                .get(STRICT_TRANSPORT_SECURITY)
                .is_none(),
            "an inferred scheme must not pin the name for a year"
        );
        assert!(
            applied_for(stated(Scheme::Https), Some(&secure_origin()))
                .get(STRICT_TRANSPORT_SECURITY)
                .is_some(),
            "a hop that stated https is still the case HSTS exists for"
        );
    }

    #[test]
    fn applying_twice_leaves_one_value_per_header() {
        let mut headers = HeaderMap::new();
        let policy = ContentSecurityPolicy::default();
        let origin = secure_origin();
        apply_security_headers(&mut headers, &policy, stated(Scheme::Https), Some(&origin));
        apply_security_headers(&mut headers, &policy, stated(Scheme::Https), Some(&origin));
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
        let origin = secure_origin();
        apply_security_headers(&mut headers, &policy, stated(Scheme::Https), Some(&origin));
        apply_security_headers(&mut headers, &policy, stated(Scheme::Http), Some(&origin));
        assert!(headers.get(STRICT_TRANSPORT_SECURITY).is_none());
    }
}
