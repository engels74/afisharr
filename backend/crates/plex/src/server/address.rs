// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where the server is, as a value that cannot be half a URL.

use thiserror::Error;
use url::Url;

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

/// What a redacted password is rendered as.
///
/// Not the empty string: an address that showed `http://admin@plex.lan` would
/// read as one configured without a password at all, and the operator checking
/// why their proxy refuses the request needs to see that one is being sent.
const REDACTED: &str = "***";

/// `text` with any password in it replaced by [`REDACTED`].
///
/// A base address is whatever the operator configured, and an operator whose
/// server sits behind a reverse proxy configures `http://user:secret@plex.lan`
/// — a secret this build then holds in a string that is displayed, logged, and
/// returned to the browser. The password is kept for the request and removed
/// from every rendering of it.
///
/// Takes text rather than an address, so that a base which never parsed is
/// covered too: the failure messages in [`AddressError`] quote what the
/// operator typed, and that is the one rendering most likely to be pasted into
/// a bug report.
#[must_use]
pub fn redact_credentials(text: &str) -> String {
    match Url::parse(text.trim()) {
        Ok(mut url) if url.password().is_some() => {
            if url.set_password(Some(REDACTED)).is_ok() {
                return url.into();
            }
            scrub(text)
        }
        // Nothing to hide, and the text is returned exactly as it arrived: a
        // round trip through `Url` would normalise an address the operator has
        // to recognise.
        Ok(_) => text.to_owned(),
        // Not a URL, which does not mean not a credential: `http://u:p@ plex`
        // fails to parse and still names a password.
        Err(_) => scrub(text),
    }
}

/// The same redaction, by hand, for text `Url` will not parse.
fn scrub(text: &str) -> String {
    let Some((before, rest)) = text.split_once("://") else {
        return text.to_owned();
    };
    // The authority ends where the path, query, or fragment begins; a `@` past
    // that point belongs to some other part of the text.
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let Some((userinfo, host)) = authority.rsplit_once('@') else {
        return text.to_owned();
    };
    match userinfo.split_once(':') {
        Some((user, _)) => format!("{before}://{user}:{REDACTED}@{host}{tail}"),
        // A username and no password is not a secret.
        None => text.to_owned(),
    }
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
    fn redaction_covers_the_text_of_an_address_that_never_parsed() {
        // `AddressError` quotes what the operator typed, and that message is
        // the rendering most likely to be pasted into a bug report.
        let error = ServerAddress::parse("http://admin:hunter2@ plex.lan")
            .expect_err("a space is not a host");
        let detail = redact_credentials(&error.to_string());
        assert!(!detail.contains("hunter2"), "{detail}");
        assert!(detail.contains("admin:***@"), "{detail}");
    }

    #[test]
    fn redaction_leaves_an_address_with_nothing_to_hide_exactly_as_it_arrived() {
        // Byte for byte: an operator checking the address on the page has to
        // recognise what they typed, and a round trip through `Url` would
        // normalise it under them.
        for text in [
            "http://plex.lan:32400",
            "https://home.example/pms/",
            "http://admin@plex.lan",
            "not an address at all",
        ] {
            assert_eq!(redact_credentials(text), text);
        }
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
