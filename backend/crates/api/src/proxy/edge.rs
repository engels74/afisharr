// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading the trusted chain, right to left.

use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;

use crate::proxy::{
    Scheme, TrustedProxies,
    peer::{FORWARDED_FOR, FORWARDED_PROTO},
};

/// What the trusted chain said, and how much of it the chain wrote.
///
/// Read right to left, never left to right. The leftmost entry of any
/// forwarded header is whatever the client wrote, and a trusted proxy that
/// *appends* — which is what every mainstream proxy does by default, including
/// nginx's `proxy_add_x_forwarded_for` — leaves the forged entry sitting in
/// front of the real one. Trusting the leftmost therefore hands the caller the
/// address every limit is counted against and every audit line records, which
/// is `I-SEC-1` failing while reporting that it works.
pub(super) struct Edge {
    /// The client address, when the walk could prove one.
    pub(super) address: Option<IpAddr>,
    /// How many entries at the right of a forwarded header this instance can
    /// attribute to proxies it trusts.
    ///
    /// Each hop appends one entry, so the entry the *client-facing* proxy
    /// wrote sits exactly this far from the right — and it sits there however
    /// many entries the client prepended, which is why every forwarded header
    /// is indexed from the right and none of them from the left.
    hops: usize,
}

impl Edge {
    /// Walks `X-Forwarded-For` from the right, discarding entries that are
    /// themselves configured proxies, and stops at the first one that is not.
    /// That entry is the address the last trustworthy hop actually saw.
    ///
    /// An entry that is not an address at all ends the walk with nothing — the
    /// chain cannot be shown to be trusted past a value that cannot be
    /// compared, and the peer's own address, with the peer's own hop, is the
    /// safe answer (P2). A walk that reaches the left end without finding an
    /// untrusted entry ends the same way, and for the same reason: it has not
    /// found a client, and the leftmost entry is whatever the caller prepended.
    pub(super) fn resolve(headers: &HeaderMap, trusted: &TrustedProxies) -> Self {
        let chain = entries(headers, FORWARDED_FOR);
        let unprovable = Self {
            address: None,
            // The immediate peer is trusted — that is why this code is running
            // — and it wrote the last entry of every header it forwarded.
            hops: 1,
        };
        if chain.is_empty() {
            return unprovable;
        }

        for (index, entry) in chain.iter().enumerate().rev() {
            let Some(address) = parse_entry(entry) else {
                return unprovable;
            };
            if !trusted.trusts(address) {
                return Self {
                    address: Some(address),
                    // This entry is the client, written by the proxy in front
                    // of it; everything to its right came from a hop this
                    // instance trusts.
                    hops: chain.len() - index,
                };
            }
        }
        // Every entry is a trusted address and the walk found no client, which
        // is not the same as the leftmost entry *being* the client. Returning
        // it was the hole: a caller inside the trusted range — a second
        // container on the same bridge network, a sidecar, the proxy host —
        // prepends `X-Forwarded-For: 10.1.2.3`, the edge appends the trusted
        // address it saw, and every entry passes. That caller then picks the
        // address every limit is counted against, so rotating one octet buys a
        // fresh `Bucket::Anonymous` counter per request and the anonymous
        // allowance bounds nothing at all, while `sessions.ip` and every audit
        // line record a value the caller chose (`I-SEC-1`).
        //
        // Nothing here can separate that from an operator whose client LAN is
        // itself inside `trustProxy`, so the answer is the same one an entry
        // that will not parse gets: the chain is not provable, and the peer's
        // own address is the safe reading (P2). The cost is stated rather than
        // hidden — an operator who trusts a whole `/8` because their proxy
        // lives in it has every client behind that proxy counted as one, which
        // is the state an instance with no `trustProxy` at all is already in,
        // and the fix is to name the proxy rather than the range it sits in.
        unprovable
    }

    /// The scheme the client-facing hop of the trusted chain observed.
    ///
    /// A proxy that appends leaves the client's own claim in front of its
    /// value, so the leftmost entry is the one value here must never take: on
    /// an HTTPS instance a forged `http` would strip `Secure` from the session
    /// cookie and drop `Strict-Transport-Security`, and on a plaintext one a
    /// forged `https` would add both to answers that cannot carry them.
    ///
    /// When the header is shorter than the chain — the common case of a proxy
    /// that overwrites rather than appends — the rightmost entry is the only
    /// one this instance can attribute to anybody, and it is the immediate
    /// peer's.
    ///
    /// One case this cannot decide, and it is stated here rather than papered
    /// over: a trusted proxy that sets `X-Forwarded-For` and leaves
    /// `X-Forwarded-Proto` alone passes the client's own claim through
    /// untouched, and the resulting one-entry chain is indistinguishable from
    /// a one-entry chain the proxy wrote itself. Counting entries cannot
    /// separate them, so nothing here tries to. What that forgery can reach is
    /// bounded elsewhere instead: `Strict-Transport-Security` — the one effect
    /// a browser remembers for a year — is emitted only when the operator has
    /// configured an `https` `publicOrigin`, which no caller can write
    /// (`security::headers`). A forged `Secure` on the session cookie is the
    /// remainder, and it is self-correcting: the browser simply withholds a
    /// cookie it was told to keep for TLS.
    pub(super) fn scheme(&self, headers: &HeaderMap) -> Scheme {
        let chain = entries(headers, FORWARDED_PROTO);
        let claimed = chain
            .len()
            .checked_sub(self.hops)
            .and_then(|index| chain.get(index))
            .or_else(|| chain.last())
            .copied()
            .unwrap_or_default();
        if claimed.eq_ignore_ascii_case("https") {
            Scheme::Https
        } else {
            Scheme::Http
        }
    }
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
fn parse_entry(entry: &str) -> Option<IpAddr> {
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
    use std::net::SocketAddr;

    use axum::http::{HeaderMap, HeaderValue};

    use super::{FORWARDED_FOR, FORWARDED_PROTO};
    use crate::proxy::{ClientContext, Scheme, TrustedProxies};

    fn peer(text: &str) -> SocketAddr {
        format!("{text}:51234").parse().expect("a valid peer")
    }

    fn headers(entries: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in entries {
            map.insert(*name, HeaderValue::from_str(value).expect("a valid header"));
        }
        map
    }

    #[test]
    fn a_forged_scheme_in_front_of_the_edge_s_own_does_not_suppress_secure() {
        // The attack: the instance is behind TLS, the client sends
        // `X-Forwarded-Proto: http`, and the proxy appends what it actually
        // saw. Reading the leftmost entry strips `Secure` from the session
        // cookie and drops HSTS on a connection that is carrying both.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[(FORWARDED_FOR, "1.2.3.4"), (FORWARDED_PROTO, "http, https")]),
            &trusted,
        );
        assert_eq!(context.scheme, Scheme::Https);
        assert!(context.scheme.is_secure());
    }

    #[test]
    fn a_forged_scheme_never_turns_a_plaintext_hop_secure() {
        // The mirror of the case above, and the reason the entry is chosen by
        // position rather than by looking for an `https` anywhere in the list.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[(FORWARDED_FOR, "1.2.3.4"), (FORWARDED_PROTO, "https, http")]),
            &trusted,
        );
        assert_eq!(context.scheme, Scheme::Http);
    }

    #[test]
    fn however_many_entries_the_client_prepends_the_edge_s_own_is_read() {
        // Indexing from the right is what makes this hold: the client controls
        // how long the forged prefix is, and it controls it independently in
        // each header.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        for prefix in ["http", "http, http", "http, http, http"] {
            let context = ClientContext::resolve(
                peer("10.0.0.5"),
                &headers(&[
                    (FORWARDED_FOR, "1.2.3.4"),
                    (FORWARDED_PROTO, &format!("{prefix}, https")),
                ]),
                &trusted,
            );
            assert_eq!(context.scheme, Scheme::Https, "prefix {prefix:?}");
        }
    }

    #[test]
    fn two_appending_hops_report_the_scheme_the_client_facing_one_saw() {
        // Client → edge over TLS → an internal hop in plaintext → here. The
        // rightmost entry is the internal hop's, and answering `http` on it
        // would strip `Secure` from a cookie travelling over TLS.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[
                (FORWARDED_FOR, "1.2.3.4, 10.0.0.9"),
                (FORWARDED_PROTO, "https, http"),
            ]),
            &trusted,
        );
        assert_eq!(context.address.to_string(), "1.2.3.4");
        assert_eq!(context.scheme, Scheme::Https);
    }

    #[test]
    fn a_proxy_that_overwrites_rather_than_appends_is_read_as_it_always_was() {
        // The overwhelmingly common configuration: one entry, written by the
        // edge, whatever the client sent.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[
                (FORWARDED_FOR, "1.2.3.4, 10.0.0.9"),
                (FORWARDED_PROTO, "https"),
            ]),
            &trusted,
        );
        assert_eq!(context.scheme, Scheme::Https);
    }

    #[test]
    fn a_chain_that_cannot_be_walked_reads_the_scheme_from_the_peer_s_own_entry() {
        // "unknown" ends the walk, so the only hop this instance can attribute
        // anything to is the peer — and the peer wrote the last entry.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[
                (FORWARDED_FOR, "1.2.3.4, unknown"),
                (FORWARDED_PROTO, "https, http"),
            ]),
            &trusted,
        );
        assert_eq!(context.address.to_string(), "10.0.0.5");
        assert_eq!(context.scheme, Scheme::Http);
    }

    #[test]
    fn an_entry_carrying_a_port_is_still_the_client_it_names() {
        // Azure Application Gateway and App Service append `host:port`, and
        // several `HAProxy` configurations do too. Refusing the entry ends the
        // walk, and every client on the internet is then attributed to the
        // proxy — one rate-limit counter for all of them, one address in the
        // session list, and no sign the configuration did not take.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[(FORWARDED_FOR, "203.0.113.9:51234")]),
            &trusted,
        );
        assert_eq!(context.address.to_string(), "203.0.113.9");
    }

    #[test]
    fn a_bracketed_ipv6_entry_is_read_with_and_without_its_port() {
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        for entry in ["[2001:db8::1]", "[2001:db8::1]:51234", "2001:db8::1"] {
            let context = ClientContext::resolve(
                peer("10.0.0.5"),
                &headers(&[(FORWARDED_FOR, entry)]),
                &trusted,
            );
            assert_eq!(
                context.address.to_string(),
                "2001:db8::1",
                "entry {entry:?}"
            );
        }
    }

    #[test]
    fn a_value_that_names_no_address_still_ends_the_walk() {
        // The bound on the tolerance above: `unknown` cannot be compared
        // against the trusted list, so nothing past it is provable and the
        // peer's own address is the safe answer (P2).
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        for entry in ["unknown", "_hidden", "203.0.113.9:notaport", "[2001:db8::1"] {
            let context = ClientContext::resolve(
                peer("10.0.0.5"),
                &headers(&[(FORWARDED_FOR, entry)]),
                &trusted,
            );
            assert_eq!(context.address.to_string(), "10.0.0.5", "entry {entry:?}");
        }
    }

    #[test]
    fn a_port_bearing_proxy_entry_is_matched_against_the_trusted_list() {
        // The entry is trusted, so the walk must step past it to the client
        // rather than stopping and attributing the request to the proxy.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[(FORWARDED_FOR, "203.0.113.9:4000, 10.0.0.9:8080")]),
            &trusted,
        );
        assert_eq!(context.address.to_string(), "203.0.113.9");
    }

    #[test]
    fn a_scheme_that_is_not_https_is_not_secure_however_it_is_spelled() {
        let trusted = TrustedProxies::parse(&["10.0.0.5"]).expect("parses");
        for claimed in ["", "http", "ws", "https-ish", " HTTPS "] {
            let context = ClientContext::resolve(
                peer("10.0.0.5"),
                &headers(&[(FORWARDED_PROTO, claimed)]),
                &trusted,
            );
            let expected = if claimed.trim().eq_ignore_ascii_case("https") {
                Scheme::Https
            } else {
                Scheme::Http
            };
            assert_eq!(context.scheme, expected, "claimed {claimed:?}");
        }
    }
}
