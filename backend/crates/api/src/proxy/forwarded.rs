// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading a forwarded header, without deciding anything from it.
//!
//! Split from [`super::edge`] because the two answer different questions.
//! `edge` decides *whose* claim to believe, which is the whole of `I-SEC-1`.
//! This decides only what the header says: how a list arrives, which shapes an
//! entry comes in, and which position holds the client-facing hop's value. Its
//! callers are `edge` and [`super::configured_origin`], and both must read the
//! same entry — two readers of one header are two chances to disagree about one
//! chain (P7).

use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;

use crate::proxy::{peer::FORWARDED_PROTO, trusted::canonical};

/// What the forwarded chain said about the scheme, resolved once.
///
/// One value rather than two readings of one header. [`super::edge::Edge`]
/// turns it into the request's scheme and
/// [`crate::proxy::ClientContext::at_configured_origin`] asks it whether any hop
/// stated anything at all — and the two asking the header separately is two
/// chances to disagree about one chain (P7). It is what
/// [`crate::proxy::ClientContext`] carries, in place of the hop count that used
/// to be re-read there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Claim {
    /// No hop stated a scheme. Not the same as a hop stating plaintext.
    Silent,
    /// The chain states the client was served over plaintext.
    Plaintext,
    /// The chain states the client was served over TLS.
    Tls,
}

impl Claim {
    /// The weakest claim the whole `X-Forwarded-Proto` chain supports.
    ///
    /// The reading for a chain no walk could pin a client in: an untrusted peer,
    /// whose header is worth nothing on its own, and a trusted walk that reached
    /// the left end without finding a client. Neither gives a reason to prefer
    /// one entry, and indexing anyway lands on the *nearest* hop — the entry a
    /// plaintext edge behind an internal TLS hop leaves last, which is exactly
    /// the upgrade that must not happen.
    ///
    /// So TLS is answered only when every entry says TLS. The chain a proxy
    /// overwrites is one entry and reads as it always did; prepending anything
    /// only weakens the answer, so nothing here can be forged upward.
    fn weakest(headers: &HeaderMap) -> Self {
        let chain = entries(headers, FORWARDED_PROTO);
        if chain.is_empty() {
            Self::Silent
        } else if chain.iter().copied().all(is_https) {
            Self::Tls
        } else {
            Self::Plaintext
        }
    }

    /// The claim of the entry `hops` from the right, which a proven walk found.
    fn at(headers: &HeaderMap, hops: usize) -> Self {
        match stated_scheme(headers, hops) {
            None => Self::Silent,
            Some(entry) if is_https(entry) => Self::Tls,
            Some(_) => Self::Plaintext,
        }
    }

    /// The claim a chain makes, given whether its walk proved anything.
    pub(super) fn of(headers: &HeaderMap, proven: Option<usize>) -> Self {
        proven.map_or_else(|| Self::weakest(headers), |hops| Self::at(headers, hops))
    }

    /// Whether the chain states the client was served over TLS.
    pub(crate) fn is_tls(self) -> bool {
        matches!(self, Self::Tls)
    }

    /// Whether the chain states the client was served over plaintext.
    ///
    /// Not the negation of [`Self::is_tls`]: [`Self::Silent`] is neither, and
    /// the whole of `at_configured_origin` turns on that difference.
    pub(crate) fn is_plaintext(self) -> bool {
        matches!(self, Self::Plaintext)
    }
}

/// The `X-Forwarded-Proto` entry the client-facing hop wrote, if it wrote one.
///
/// [`super::edge::Edge::scheme`] resolves the scheme from it, and
/// [`crate::proxy::ClientContext::at_configured_origin`] asks the same entry
/// whether any hop stated a scheme at all. Reading the rightmost one there
/// instead read the nearest hop rather than the client-facing one, so a two-hop
/// chain arriving as `http, https` was upgraded to HTTPS and marked as a scheme
/// the chain had vouched for — which set a `Secure` cookie and emitted
/// `Strict-Transport-Security` over a connection the client-facing hop had just
/// said was plaintext.
///
/// `None` is "no hop said anything", which is not the same as "a hop said
/// plaintext" — the whole of `at_configured_origin` turns on that difference.
///
/// When the header is shorter than the chain — the common case of a proxy that
/// overwrites rather than appends — the rightmost entry is the only one this
/// instance can attribute to anybody, and it is the immediate peer's.
fn stated_scheme(headers: &HeaderMap, hops: usize) -> Option<&str> {
    let chain = entries(headers, FORWARDED_PROTO);
    chain
        .len()
        .checked_sub(hops)
        .and_then(|index| chain.get(index))
        .or_else(|| chain.last())
        .copied()
}

/// Whether one forwarded entry names TLS.
fn is_https(claimed: &str) -> bool {
    claimed.eq_ignore_ascii_case("https")
}

/// The address one `X-Forwarded-For` entry names, in the three shapes real
/// proxies write it.
///
/// A bare address is the common one, and it was the only one this understood.
/// The other two are not exotic: Azure Application Gateway and App Service
/// append `host:port`, several `HAProxy` configurations do the same, and a proxy
/// forwarding an IPv6 client commonly brackets the literal. Refusing those ends
/// the walk, and the walk ending is not a small failure — the whole chain falls
/// back to the peer, so an operator who configured `trustProxy` correctly still
/// has every client on the internet attributed to their proxy's one address,
/// with one rate-limit counter for all of them and one address filling the
/// session list, and nothing anywhere saying the configuration did not take.
///
/// A value that names no address at all — `unknown`, an obfuscated identifier —
/// still ends the walk. That is the honest answer: the chain cannot be shown to
/// be trusted past a value that cannot be compared against the trusted list.
///
/// The address is canonicalised on the way out, so a proxy that writes
/// `::ffff:1.2.3.4` and one that writes `1.2.3.4` name the same client to every
/// reader — the trusted-list test, the rate-limit key, and the session row
/// (see [`crate::proxy::canonical`]).
pub(super) fn parse_entry(entry: &str) -> Option<IpAddr> {
    if let Ok(address) = entry.parse::<IpAddr>() {
        return Some(canonical(address));
    }
    // `1.2.3.4:51234` and `[2001:db8::1]:51234`, which `SocketAddr` reads whole.
    if let Ok(socket) = entry.parse::<SocketAddr>() {
        return Some(canonical(socket.ip()));
    }
    // `[2001:db8::1]`, a bracketed literal carrying no port.
    entry
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .and_then(|inner| inner.parse::<IpAddr>().ok())
        .map(canonical)
}

/// Every value of `name`, in order.
///
/// A chain can arrive as one comma-joined header or as several, and a proxy
/// that appends a second header line is appending to the same list.
pub(super) fn entries<'h>(headers: &'h HeaderMap, name: &str) -> Vec<&'h str> {
    headers
        .get_all(name)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(entries: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in entries {
            map.insert(*name, HeaderValue::from_str(value).expect("a valid header"));
        }
        map
    }

    #[test]
    fn an_entry_is_read_with_a_port_and_with_brackets() {
        // Azure Application Gateway and App Service append `host:port`, and
        // several `HAProxy` configurations do too. Refusing an entry ends the
        // walk, and every client on the internet is then attributed to the
        // proxy — one rate-limit counter for all of them, one address in the
        // session list, and no sign the configuration did not take.
        for entry in ["2001:db8::1", "[2001:db8::1]", "[2001:db8::1]:51234"] {
            assert_eq!(
                parse_entry(entry).expect("an address").to_string(),
                "2001:db8::1",
                "entry {entry:?}"
            );
        }
        assert_eq!(
            parse_entry("203.0.113.9:51234")
                .expect("an address")
                .to_string(),
            "203.0.113.9"
        );
    }

    #[test]
    fn a_mapped_entry_names_the_same_client_as_its_plain_spelling() {
        // Otherwise one caller is two rate-limit counters and two rows in the
        // operator's session list, depending on which spelling the hop wrote.
        for entry in ["::ffff:203.0.113.9", "[::ffff:203.0.113.9]:51234"] {
            assert_eq!(
                parse_entry(entry).expect("an address").to_string(),
                "203.0.113.9",
                "entry {entry:?}"
            );
        }
    }

    #[test]
    fn a_value_that_names_no_address_is_not_one() {
        // The bound on the tolerance above: these cannot be compared against
        // the trusted list, so nothing past them is provable (P2).
        for entry in ["unknown", "_hidden", "203.0.113.9:notaport", "[2001:db8::1"] {
            assert_eq!(parse_entry(entry), None, "entry {entry:?}");
        }
    }

    #[test]
    fn a_chain_arrives_the_same_however_the_proxy_wrote_it() {
        // One comma-joined header and two header lines are one list.
        let joined = headers(&[(FORWARDED_PROTO, "http, https")]);
        assert_eq!(entries(&joined, FORWARDED_PROTO), vec!["http", "https"]);

        let mut split = HeaderMap::new();
        split.append(FORWARDED_PROTO, HeaderValue::from_static("http"));
        split.append(FORWARDED_PROTO, HeaderValue::from_static("https"));
        assert_eq!(entries(&split, FORWARDED_PROTO), vec!["http", "https"]);

        // Empty entries are not entries, so a trailing comma does not shift
        // the position every reader indexes from.
        let ragged = headers(&[(FORWARDED_PROTO, "https, ")]);
        assert_eq!(entries(&ragged, FORWARDED_PROTO), vec!["https"]);
    }

    #[test]
    fn the_stated_scheme_is_the_client_facing_hops_entry() {
        let chain = headers(&[(FORWARDED_PROTO, "http, https")]);
        assert_eq!(stated_scheme(&chain, 1), Some("https"));
        assert_eq!(stated_scheme(&chain, 2), Some("http"));

        // Shorter than the chain: the rightmost entry is the only one that can
        // be attributed to anybody, and it is the immediate peer's.
        assert_eq!(stated_scheme(&chain, 3), Some("https"));

        // Absent is not a claim, and it is not plaintext either.
        assert_eq!(stated_scheme(&HeaderMap::new(), 1), None);
    }

    #[test]
    fn a_proven_walk_reads_the_entry_at_its_own_hop() {
        let chain = headers(&[(FORWARDED_PROTO, "http, https")]);
        assert_eq!(Claim::of(&chain, Some(1)), Claim::Tls);
        assert_eq!(Claim::of(&chain, Some(2)), Claim::Plaintext);
        assert_eq!(Claim::of(&HeaderMap::new(), Some(1)), Claim::Silent);
    }

    #[test]
    fn an_unproven_walk_takes_the_weakest_claim_in_the_chain() {
        // The hop count is a floor of one there, so indexing by it reads the
        // nearest hop — a plaintext edge behind an internal TLS hop then set a
        // `Secure` cookie the browser discards and a year of HSTS with nothing
        // to click through.
        for (chain, expected) in [
            ("http, https", Claim::Plaintext),
            ("https, http", Claim::Plaintext),
            ("https", Claim::Tls),
            ("https, https", Claim::Tls),
            ("http", Claim::Plaintext),
        ] {
            assert_eq!(
                Claim::of(&headers(&[(FORWARDED_PROTO, chain)]), None),
                expected,
                "chain {chain:?}"
            );
        }
        assert_eq!(Claim::of(&HeaderMap::new(), None), Claim::Silent);
    }

    #[test]
    fn silence_is_neither_tls_nor_plaintext() {
        assert!(!Claim::Silent.is_tls());
        assert!(!Claim::Silent.is_plaintext());
        assert!(Claim::Tls.is_tls());
        assert!(Claim::Plaintext.is_plaintext());
    }

    #[test]
    fn a_scheme_is_only_https_when_it_says_so() {
        assert!(is_https("https"));
        assert!(is_https("HTTPS"));
        for claimed in ["", "http", "ws", "https-ish"] {
            assert!(!is_https(claimed), "claimed {claimed:?}");
        }
    }
}
