// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The address operators reach this instance at, as its operator configured it.
//!
//! Every other fact about a request's origin in this module comes from the
//! request. This one cannot: `Host` is written by whoever is calling, and a
//! value the caller chooses is a value an attacker chooses. Anything Afisharr
//! signs its own name to — a return target embedded in a plex.tv sign-in, and
//! whatever else later needs an absolute URL for this instance — is judged
//! against this and against nothing the request carries (`I-SEC-1`).

use thiserror::Error;
use url::Url;

/// Why a `publicOrigin` could not be read.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum PublicOriginError {
    /// The value is not an absolute URL.
    #[error("'{0}' is not an absolute URL like https://afisharr.example")]
    Malformed(String),

    /// The value parsed, but names no host — `mailto:`, `data:`, and the rest.
    #[error("'{0}' names no host, so it cannot be an origin")]
    Opaque(String),
}

/// The origin this instance is reached at.
///
/// Held as a parsed [`Url`] and compared as an origin, so `https://host` and
/// `https://host:443` are one instance and a path, query, or fragment on
/// either side changes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicOrigin(Url);

impl PublicOrigin {
    /// Reads the configured value.
    ///
    /// # Errors
    /// Returns [`PublicOriginError`] naming the value when it is not an
    /// absolute URL, or is one with no host. Refused rather than ignored: an
    /// operator who mistyped it would otherwise get an instance that quietly
    /// refuses every hosted sign-in with nothing saying why.
    pub fn parse(value: &str) -> Result<Self, PublicOriginError> {
        let parsed =
            Url::parse(value.trim()).map_err(|_| PublicOriginError::Malformed(value.to_owned()))?;
        if !parsed.origin().is_tuple() {
            return Err(PublicOriginError::Opaque(value.to_owned()));
        }
        Ok(Self(parsed))
    }

    /// Whether `target` is a URL on this origin.
    ///
    /// An opaque origin — `javascript:`, `data:` — is not an origin and never
    /// matches, which is the case that turns a redirect into script execution.
    #[must_use]
    pub fn covers(&self, target: &str) -> bool {
        let Ok(target) = Url::parse(target) else {
            return false;
        };
        target.origin().is_tuple() && target.origin() == self.0.origin()
    }

    /// Whether `host` — a `Host` header's value — names this origin.
    ///
    /// Compared as an origin rather than as text, so `media.example` and
    /// `media.example:443` are one instance under `https` and differ under
    /// `http`. The scheme comes from the configured origin and never from the
    /// request, because the request has none to give: a `Host` header is a host
    /// and a port, and reading a scheme into it would be reading one out of
    /// thin air.
    #[must_use]
    pub fn matches_host(&self, host: &str) -> bool {
        let Ok(parsed) = Url::parse(&format!("{}://{host}", self.0.scheme())) else {
            return false;
        };
        parsed.origin().is_tuple() && parsed.origin() == self.0.origin()
    }

    /// Whether the operator configured this instance as reachable over TLS.
    ///
    /// The one statement about the scheme that no caller can write. Everything
    /// else this module knows about how a request arrived comes from a header,
    /// and a header is worth exactly as much as the trust list in front of it.
    #[must_use]
    pub fn is_secure(&self) -> bool {
        self.0.scheme() == "https"
    }

    /// The origin as configured, for a message that has to name it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_on_the_configured_origin_is_covered() {
        let origin = PublicOrigin::parse("https://afisharr.example").expect("a valid origin");
        assert!(origin.covers("https://afisharr.example/login"));
        assert!(origin.covers("https://afisharr.example/login?next=1#top"));
    }

    #[test]
    fn a_target_anywhere_else_is_not() {
        // The hole this closes: the caller posts a `forwardUrl`, the endpoint
        // embeds it in a genuine `app.plex.tv/auth` URL, and whoever finishes
        // the sign-in lands on the attacker's page.
        let origin = PublicOrigin::parse("https://afisharr.example").expect("a valid origin");
        for target in [
            "https://evil.example",
            "https://evil.example/afisharr.example",
            "https://afisharr.example.evil.example/login",
            "https://afisharr.example@evil.example/login",
            "http://afisharr.example/login",
            "//evil.example/login",
            "/login",
            "javascript:alert(1)",
            "data:text/html,<script></script>",
            "not a url",
        ] {
            assert!(
                !origin.covers(target),
                "{target} must not be treated as this instance"
            );
        }
    }

    #[test]
    fn a_default_port_and_its_spelling_are_one_instance() {
        let origin = PublicOrigin::parse("https://afisharr.example").expect("a valid origin");
        assert!(origin.covers("https://afisharr.example:443/login"));

        let explicit = PublicOrigin::parse("http://afisharr.example:8484").expect("a valid origin");
        assert!(explicit.covers("http://afisharr.example:8484/login"));
        assert!(!explicit.covers("http://afisharr.example/login"));
    }

    #[test]
    fn a_configured_path_does_not_narrow_the_origin() {
        // An origin is a scheme, a host, and a port. A subpath deployment
        // configures the URL it is served from, and every path on that origin
        // is still this instance.
        let origin =
            PublicOrigin::parse("https://media.example/afisharr/").expect("a valid origin");
        assert!(origin.covers("https://media.example/anything"));
    }

    #[test]
    fn a_host_header_naming_the_configured_origin_matches() {
        let origin = PublicOrigin::parse("https://afisharr.example").expect("a valid origin");
        assert!(origin.matches_host("afisharr.example"));
        assert!(origin.matches_host("AFISHARR.EXAMPLE"));
        // The default port for the configured scheme, spelled out.
        assert!(origin.matches_host("afisharr.example:443"));
    }

    #[test]
    fn a_host_header_naming_anywhere_else_does_not_match() {
        // The `Host` header is written by whoever is calling. Matching it
        // against the configured origin is what makes a proxy that rewrites
        // `Host` stop breaking every write, without letting a caller nominate
        // an origin of their own.
        let origin = PublicOrigin::parse("https://afisharr.example").expect("a valid origin");
        for host in [
            "evil.example",
            "afisharr.example.evil.example",
            "afisharr.example:8484",
            "",
            "not a host",
        ] {
            assert!(!origin.matches_host(host), "'{host}' must not match");
        }
    }

    #[test]
    fn the_configured_scheme_is_what_says_whether_the_instance_is_reached_over_tls() {
        assert!(
            PublicOrigin::parse("https://afisharr.example")
                .expect("a valid origin")
                .is_secure()
        );
        assert!(
            !PublicOrigin::parse("http://192.168.1.10:8484")
                .expect("a valid origin")
                .is_secure()
        );
    }

    #[test]
    fn a_value_that_is_not_an_absolute_url_is_refused() {
        for value in ["afisharr.example", "", "   ", "/login"] {
            assert!(
                matches!(
                    PublicOrigin::parse(value),
                    Err(PublicOriginError::Malformed(_))
                ),
                "'{value}' must not be read as an origin"
            );
        }
    }

    #[test]
    fn a_url_with_no_host_is_refused() {
        assert!(matches!(
            PublicOrigin::parse("mailto:operator@afisharr.example"),
            Err(PublicOriginError::Opaque(_))
        ));
    }
}
