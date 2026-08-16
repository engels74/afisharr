// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading what a request actually asked for.
//!
//! Two things live here because a real server reads both and the fake read
//! neither properly: the query string as *ordered pairs* rather than a map,
//! and the container window from the headers as well as from the query.
//!
//! **Pairs, not a map.** A map collapses a repeated key onto its last value,
//! which is right for the single-valued arguments most handlers read and wrong
//! for the ones Plex repeats: a conjunctive filter sends one `genre&=` per
//! value. Read from a map, a request asking for two genres would ask for one
//! and the fake would answer confidently.
//!
//! **Headers as well as query.** `python-plexapi` sets `X-Plex-Container-Start`
//! and `X-Plex-Container-Size` as headers on every paging loop
//! (`plexapi/base.py:346-350`) and as query arguments elsewhere
//! (`plexapi/library.py:519-520`). A real server honours both; a fake that
//! honoured one answers the whole library to half its callers.

use std::convert::Infallible;

use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, request::Parts},
};

/// The header a token is presented in.
const TOKEN_HEADER: &str = "x-plex-token";

/// Where a window starts.
const START: &str = "X-Plex-Container-Start";

/// How wide a window is.
const SIZE: &str = "X-Plex-Container-Size";

/// One request's query string, in the order it arrived.
#[derive(Debug, Clone, Default)]
pub(crate) struct Arguments(Vec<(String, String)>);

impl Arguments {
    /// Parses one raw query string.
    pub(crate) fn parse(query: Option<&str>) -> Self {
        Self(
            url::form_urlencoded::parse(query.unwrap_or_default().as_bytes())
                .map(|(name, value)| (name.into_owned(), value.into_owned()))
                .collect(),
        )
    }

    /// The first value under `name`.
    pub(crate) fn first(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Every value under `name`, in order.
    pub(crate) fn all(&self, name: &str) -> Vec<&str> {
        self.0
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
            .collect()
    }

    /// Whether a flag argument is set to `1`.
    pub(crate) fn flag(&self, name: &str) -> bool {
        self.first(name).is_some_and(|value| value == "1")
    }

    /// Every pair, for the handlers that read the whole query.
    pub(crate) fn pairs(&self) -> &[(String, String)] {
        &self.0
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Arguments {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self::parse(parts.uri.query()))
    }
}

/// The window a listing call was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Paging {
    /// The offset of the first row wanted.
    pub(crate) start: usize,
    /// How many rows to return.
    pub(crate) size: usize,
}

impl Paging {
    /// Reads the window from the headers, then from the query.
    ///
    /// The query wins where both are set, because that is the more specific of
    /// the two: a client that set a header once for a session and an argument
    /// for one call means the argument.
    pub(crate) fn of(headers: &HeaderMap, arguments: &Arguments) -> Self {
        let read = |name: &str| -> Option<usize> {
            arguments
                .first(name)
                .and_then(|value| value.parse().ok())
                .or_else(|| {
                    headers
                        .get(name)
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse().ok())
                })
        };
        Self {
            start: read(START).unwrap_or(0),
            // No window means the whole result, which is what a real server
            // does. A fake that defaulted to a page size would answer a short
            // library to a caller that asked for all of it.
            size: read(SIZE).unwrap_or(usize::MAX),
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Paging {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self::of(
            &parts.headers,
            &Arguments::parse(parts.uri.query()),
        ))
    }
}

/// The token a request presented, from the header or the query.
///
/// Both, because a real server accepts both and a browser embedding an image
/// URL can only use the second.
///
/// Emptiness is checked on each spelling before falling back, not on the
/// winner: a request carrying an empty header alongside a usable query argument
/// is a request that presented a token, and letting the empty one shadow the
/// fallback would refuse it as though it had presented none.
pub(crate) fn token(headers: &HeaderMap, arguments: &Arguments) -> Option<String> {
    headers
        .get(TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            arguments
                .first("X-Plex-Token")
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, HeaderValue::from_str(value).expect("a valid header"));
        }
        map
    }

    #[test]
    fn a_repeated_parameter_keeps_every_value_it_was_sent() {
        let read = Arguments::parse(Some("genre%26=93&genre%26=94&id=1001"));
        assert_eq!(read.all("genre&"), ["93", "94"]);
        assert_eq!(read.first("id"), Some("1001"));
        assert_eq!(read.first("missing"), None);
    }

    #[test]
    fn a_percent_encoded_value_is_decoded_the_way_a_server_reads_it() {
        assert_eq!(
            Arguments::parse(Some("title.value=a+b%26c")).first("title.value"),
            Some("a b&c")
        );
    }

    #[test]
    fn a_window_is_read_from_the_query_arguments() {
        let arguments =
            Arguments::parse(Some("X-Plex-Container-Start=200&X-Plex-Container-Size=50"));
        assert_eq!(
            Paging::of(&HeaderMap::new(), &arguments),
            Paging {
                start: 200,
                size: 50
            }
        );
    }

    #[test]
    fn a_window_is_read_from_the_headers_too() {
        // The reference client pages by header on every loop, so a fake that
        // read only the query answered it the whole library every time.
        let paged = headers(&[
            ("x-plex-container-start", "100"),
            ("x-plex-container-size", "25"),
        ]);
        assert_eq!(
            Paging::of(&paged, &Arguments::default()),
            Paging {
                start: 100,
                size: 25
            }
        );
    }

    #[test]
    fn a_query_argument_wins_over_a_header() {
        let paged = headers(&[("x-plex-container-size", "25")]);
        assert_eq!(
            Paging::of(&paged, &Arguments::parse(Some("X-Plex-Container-Size=5"))).size,
            5
        );
    }

    #[test]
    fn a_request_with_no_window_asks_for_everything() {
        assert_eq!(
            Paging::of(&HeaderMap::new(), &Arguments::default()),
            Paging {
                start: 0,
                size: usize::MAX
            }
        );
    }

    #[test]
    fn a_window_that_is_not_a_number_falls_back_rather_than_failing() {
        // A real server ignores what it cannot read here. The fake matching
        // that keeps a client's malformed request a client bug rather than a
        // fake-only failure.
        let arguments = Arguments::parse(Some("X-Plex-Container-Start=soon"));
        assert_eq!(Paging::of(&HeaderMap::new(), &arguments).start, 0);
    }

    #[test]
    fn a_token_is_read_from_either_place_a_client_puts_it() {
        assert_eq!(
            token(&headers(&[("x-plex-token", "abc")]), &Arguments::default()).as_deref(),
            Some("abc")
        );
        assert_eq!(
            token(
                &HeaderMap::new(),
                &Arguments::parse(Some("X-Plex-Token=abc"))
            )
            .as_deref(),
            Some("abc")
        );
        assert_eq!(token(&HeaderMap::new(), &Arguments::default()), None);
        assert_eq!(
            token(&headers(&[("x-plex-token", "")]), &Arguments::default()),
            None,
            "an empty token is no token"
        );
        assert_eq!(
            token(
                &headers(&[("x-plex-token", "")]),
                &Arguments::parse(Some("X-Plex-Token=abc"))
            )
            .as_deref(),
            Some("abc"),
            "an empty header does not shadow a usable query argument"
        );
    }
}
