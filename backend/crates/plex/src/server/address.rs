// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where the server is, as a value that cannot be half a URL.

use thiserror::Error;
use url::Url;

use crate::server::redact_credentials;

/// Why an address could not be built.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AddressError {
    /// The text is not a URL at all.
    #[error("'{text}' is not a URL")]
    Unparseable {
        /// What the operator typed.
        text: String,
        /// The underlying failure.
        #[source]
        source: url::ParseError,
    },

    /// The URL names a scheme this client does not speak.
    ///
    /// The text is carried alongside the scheme because the commonest way to
    /// reach this is not an exotic scheme at all: `plex.lan:32400` parses as
    /// the scheme `plex.lan`, and an operator told only that is left staring at
    /// a hostname being called a protocol.
    #[error(
        "'{text}' does not begin with http:// or https://, so Afisharr read \
         '{scheme}' as its scheme"
    )]
    UnsupportedScheme {
        /// What the operator typed.
        text: String,
        /// The scheme found.
        scheme: String,
    },

    /// The URL names no host.
    #[error("'{text}' names no host")]
    NoHost {
        /// What the operator typed.
        text: String,
    },
}

/// The base address of one Plex Media Server.
///
/// Parsed once, at the point the operator supplies it, so no call site ever
/// concatenates strings into a URL. The query and fragment are dropped rather
/// than carried: a base of `http://plex:32400/?X-Plex-Token=leaked` would
/// otherwise put a token from somewhere else on every request this client
/// makes, and `Url::join` would silently discard it anyway — silently being
/// the part that makes it a bug.
///
/// Any password stays on the URL requests are built from and appears in none
/// of its renderings; see [`ServerAddress::as_str`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerAddress {
    /// The address requests are composed against, credentials included.
    url: Url,
    /// The same address with any password redacted, computed once.
    shown: Box<str>,
}

impl ServerAddress {
    /// Parses `text` as a server address.
    ///
    /// # Errors
    /// Returns [`AddressError`] when the text is not a URL, names a scheme
    /// other than `http`/`https`, or names no host.
    pub fn parse(text: &str) -> Result<Self, AddressError> {
        let mut url = Url::parse(text.trim()).map_err(|source| AddressError::Unparseable {
            text: text.to_owned(),
            source,
        })?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AddressError::UnsupportedScheme {
                text: text.to_owned(),
                scheme: url.scheme().to_owned(),
            });
        }
        if url.host_str().is_none() {
            return Err(AddressError::NoHost {
                text: text.to_owned(),
            });
        }
        url.set_query(None);
        url.set_fragment(None);
        // A base whose path does not end in `/` makes `Url::join` replace the
        // last segment instead of appending to it, so `http://plex:32400/pms`
        // joined with `identity` would resolve to `http://plex:32400/identity`
        // — a different server-relative path than the operator configured.
        if !url.path().ends_with('/') {
            let path = format!("{}/", url.path());
            url.set_path(&path);
        }
        let shown = redact_credentials(url.as_str()).into_boxed_str();
        Ok(Self { url, shown })
    }

    /// The address as text, for display.
    ///
    /// Redacted, and deliberately not the text a request is built from: this
    /// is what the interface renders and what an operator pastes into a bug
    /// report, and a password put on screen there is a password disclosed.
    /// Parsing this back would present `***` to the server as the password.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.shown
    }

    /// The host, for error messages that must name what was configured (P6).
    #[must_use]
    pub fn host(&self) -> &str {
        self.url.host_str().unwrap_or("the Plex server")
    }

    /// This address with `path` appended and `query` applied.
    ///
    /// `path` is server-relative and never starts with `/`: a leading slash
    /// makes `Url::join` replace the whole path, which would discard a base
    /// path an operator configured behind a reverse proxy.
    ///
    /// # Errors
    /// Returns the parse failure, which can only mean a path this crate
    /// composed is not a valid relative reference.
    pub fn endpoint(&self, path: &str, query: &[(String, String)]) -> Result<Url, url::ParseError> {
        let mut url = self.url.join(path.trim_start_matches('/'))?;
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                pairs.append_pair(key, value);
            }
        }
        Ok(url)
    }

    /// The same composition, for a `key` the *server* supplied.
    ///
    /// Its own method because the trust is different. Every other endpoint is
    /// assembled from a literal in this crate, so it lands where this crate
    /// meant it to. A discovered key is a string out of a response body, and
    /// `Url::join` resolves an absolute one by replacing the origin — so a
    /// server that answered `http://elsewhere.example/x`, or a proxy or cache
    /// that rewrote the body to say so, would redirect the next request off
    /// this machine. That request carries the instance's `X-Plex-Token`, so
    /// the redirect is a credential handed to whoever the body named, and the
    /// same move reaches any host this instance can route to.
    ///
    /// Returns `None` for a key that does not land on the configured server —
    /// which is a fact to report, not a request to make. Three things have to
    /// match, and the origin is only the first:
    ///
    /// 1. The origin, which is what an absolute key replaces.
    /// 2. The base path, because an operator who configured
    ///    `https://home.example/pms` put Plex behind that prefix, and whatever
    ///    serves the rest of that host is somebody else's.
    /// 3. The userinfo, which the origin does *not* cover: a key of
    ///    `http://u:p@plex.lan:32400/x` has the same origin as the base and
    ///    still lets the answer choose what this instance authenticates as.
    ///
    /// # Errors
    /// Returns the parse failure when the key is not a valid reference at all.
    pub fn discovered_endpoint(&self, key: &str) -> Result<Option<Url>, url::ParseError> {
        let url = self.endpoint(key, &[])?;
        let on_this_server = url.origin() == self.url.origin()
            && url.path().starts_with(self.url.path())
            && url.username() == self.url.username()
            && url.password() == self.url.password();
        Ok(on_this_server.then_some(url))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ordinary_address_parses() {
        let address = ServerAddress::parse("http://plex.lan:32400").expect("a valid address");
        assert_eq!(address.as_str(), "http://plex.lan:32400/");
        assert_eq!(address.host(), "plex.lan");
    }

    #[test]
    fn a_token_pasted_into_the_base_does_not_survive() {
        // Configured once, it would ride every later request — including the
        // ones that carry the instance's own token — and land in whatever the
        // operator's proxy logs (D-032).
        let address = ServerAddress::parse("http://plex.lan:32400/?X-Plex-Token=leaked")
            .expect("a valid address");
        assert_eq!(address.as_str(), "http://plex.lan:32400/");
    }

    #[test]
    fn a_password_in_the_base_is_sent_to_the_server_and_shown_to_nobody() {
        // The reverse-proxy case: basic auth in front of the server. The
        // request has to carry it, and the settings page — which renders this
        // address, and hands it to the browser — must not.
        let address =
            ServerAddress::parse("http://admin:hunter2@plex.lan:32400").expect("a valid address");
        assert_eq!(address.as_str(), "http://admin:***@plex.lan:32400/");
        assert!(!address.as_str().contains("hunter2"));

        let url = address.endpoint("identity", &[]).expect("a valid endpoint");
        assert_eq!(url.password(), Some("hunter2"), "the request still has it");
        assert_eq!(url.username(), "admin");
    }

    #[test]
    fn a_base_path_is_preserved_when_an_endpoint_is_appended() {
        // The reverse-proxy case: `/pms` in front of the server. Without the
        // trailing slash `Url::join` drops the segment and the request goes to
        // a path the operator never configured.
        let address = ServerAddress::parse("https://home.example/pms").expect("a valid address");
        let url = address.endpoint("identity", &[]).expect("a valid endpoint");
        assert_eq!(url.as_str(), "https://home.example/pms/identity");
    }

    #[test]
    fn query_pairs_are_encoded_rather_than_pasted() {
        let address = ServerAddress::parse("http://plex.lan:32400").expect("a valid address");
        let url = address
            .endpoint(
                "library/sections/1/all",
                &[("title".to_owned(), "a b&c=d".to_owned())],
            )
            .expect("a valid endpoint");
        assert_eq!(
            url.as_str(),
            "http://plex.lan:32400/library/sections/1/all?title=a+b%26c%3Dd"
        );
    }

    #[test]
    fn a_discovered_key_that_lands_on_this_server_composes_the_way_a_literal_one_does() {
        let address = ServerAddress::parse("https://home.example/pms").expect("a valid address");
        let url = address
            .discovered_endpoint("/library/sections/1/genre")
            .expect("a valid reference")
            .expect("on this server");
        assert_eq!(
            url.as_str(),
            "https://home.example/pms/library/sections/1/genre"
        );
    }

    #[test]
    fn a_discovered_key_naming_another_host_is_refused_before_a_token_is_sent() {
        // The key comes out of a response body. An absolute URL in it makes
        // `Url::join` replace the origin, and the request that would follow
        // carries this instance's `X-Plex-Token` — so a compromised server, or
        // anything that rewrote its answer, could name a collector and be
        // handed the credential (D-032).
        let address = ServerAddress::parse("http://plex.lan:32400").expect("a valid address");
        for key in [
            "http://collector.example/library/sections/1/genre",
            "https://plex.lan:32400/library/sections/1/genre",
            "http://plex.lan:8080/library/sections/1/genre",
            "http://user:pass@plex.lan:32400/library/sections/1/genre",
        ] {
            assert_eq!(
                address.discovered_endpoint(key).expect("a valid reference"),
                None,
                "{key}"
            );
        }
    }

    #[test]
    fn a_discovered_key_that_escapes_the_configured_base_path_is_refused_too() {
        // Same host, outside the prefix the operator put Plex behind. Whatever
        // serves `/` on that host is not the server this client is bound to.
        let address = ServerAddress::parse("https://home.example/pms").expect("a valid address");
        assert_eq!(
            address
                .discovered_endpoint("https://home.example/library/sections/1/genre")
                .expect("a valid reference"),
            None
        );
    }

    #[test]
    fn a_protocol_relative_discovered_key_cannot_smuggle_a_host_either() {
        // `//collector.example/x` is an origin change in two characters, and
        // the leading-slash trim turns it into a path on this server instead.
        let address = ServerAddress::parse("http://plex.lan:32400").expect("a valid address");
        let url = address
            .discovered_endpoint("//collector.example/x")
            .expect("a valid reference")
            .expect("resolved onto this server");
        assert_eq!(url.host_str(), Some("plex.lan"));
    }

    #[test]
    fn a_scheme_this_client_cannot_speak_is_refused_at_the_point_it_is_typed() {
        let error = ServerAddress::parse("ftp://plex.lan").expect_err("ftp is not reachable");
        assert!(matches!(error, AddressError::UnsupportedScheme { .. }));
        assert!(error.to_string().contains("http"), "{error}");
    }

    #[test]
    fn text_that_is_not_a_url_at_all_names_what_was_typed() {
        let error = ServerAddress::parse("not an address").expect_err("not a URL");
        assert!(matches!(error, AddressError::Unparseable { .. }));
        assert!(error.to_string().contains("not an address"), "{error}");
    }

    #[test]
    fn a_host_and_port_with_no_scheme_names_what_was_typed_too() {
        // The commonest mistake, and the one that reads worst if the message
        // names only the scheme: `plex.lan:32400` parses with `plex.lan` as its
        // scheme, and an operator told that is left staring at a hostname being
        // called a protocol.
        let error = ServerAddress::parse("plex.lan:32400").expect_err("no scheme, no address");
        assert!(error.to_string().contains("plex.lan:32400"), "{error}");
        assert!(error.to_string().contains("http://"), "{error}");
    }
}
