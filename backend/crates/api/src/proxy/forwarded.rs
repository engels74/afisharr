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

use crate::proxy::peer::FORWARDED_PROTO;

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
pub(super) fn stated_scheme(headers: &HeaderMap, hops: usize) -> Option<&str> {
    let chain = entries(headers, FORWARDED_PROTO);
    chain
        .len()
        .checked_sub(hops)
        .and_then(|index| chain.get(index))
        .or_else(|| chain.last())
        .copied()
}

/// Whether one forwarded entry names TLS.
pub(super) fn is_https(claimed: &str) -> bool {
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
pub(super) fn parse_entry(entry: &str) -> Option<IpAddr> {
    if let Ok(address) = entry.parse::<IpAddr>() {
        return Some(address);
    }
    // `1.2.3.4:51234` and `[2001:db8::1]:51234`, which `SocketAddr` reads whole.
    if let Ok(socket) = entry.parse::<SocketAddr>() {
        return Some(socket.ip());
    }
    // `[2001:db8::1]`, a bracketed literal carrying no port.
    entry
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .and_then(|inner| inner.parse::<IpAddr>().ok())
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
    fn a_scheme_is_only_https_when_it_says_so() {
        assert!(is_https("https"));
        assert!(is_https("HTTPS"));
        for claimed in ["", "http", "ws", "https-ish"] {
            assert!(!is_https(claimed), "claimed {claimed:?}");
        }
    }
}
