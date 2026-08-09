// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading the trusted chain, right to left.

use std::net::IpAddr;

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
    /// safe answer (P2).
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

        let mut leftmost = None;
        for (index, entry) in chain.iter().enumerate().rev() {
            let Ok(address) = entry.parse::<IpAddr>() else {
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
            leftmost = Some(address);
        }
        // Every hop in the chain is a proxy this instance trusts, so the
        // leftmost of them is as close to the client as the header goes, and
        // the whole chain was written by the edge.
        Self {
            address: leftmost,
            hops: chain.len(),
        }
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

/// Every value of `name`, in order.
///
/// A chain can arrive as one comma-joined header or as several, and a proxy
/// that appends a second header line is appending to the same list.
fn entries<'h>(headers: &'h HeaderMap, name: &str) -> Vec<&'h str> {
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
