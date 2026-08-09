// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where a request really came from, and whether it really came over TLS.

use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;

use crate::proxy::{TrustedProxies, edge::Edge};

/// `X-Forwarded-For`, honoured only from a trusted peer.
pub(super) const FORWARDED_FOR: &str = "x-forwarded-for";

/// `X-Forwarded-Proto`, honoured only from a trusted peer.
pub(super) const FORWARDED_PROTO: &str = "x-forwarded-proto";

/// How a request reached this instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    /// Plaintext, as far as this instance can prove.
    Http,
    /// TLS, either directly or vouched for by a trusted proxy.
    Https,
}

impl Scheme {
    /// Whether `Secure` cookies and `Strict-Transport-Security` apply.
    #[must_use]
    pub const fn is_secure(self) -> bool {
        matches!(self, Self::Https)
    }

    /// How the scheme is spelled in a URL this instance builds.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }
}

/// The facts about a request's origin that every gate is keyed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientContext {
    /// The address the request is attributed to.
    ///
    /// The forwarded address when the immediate peer is a trusted proxy, and
    /// the immediate peer's own address otherwise. There is no third case, and
    /// no way to reach the forwarded value without going through here.
    pub address: IpAddr,
    /// Whether the request arrived over TLS.
    pub scheme: Scheme,
}

impl ClientContext {
    /// Resolves the origin of a request from its peer and its headers.
    ///
    /// This is the whole of `I-SEC-1`. A forwarded header from an untrusted
    /// peer is not "probably fine" and not "used with a warning" — it is
    /// discarded, and the peer's own address is what every limit is counted
    /// against.
    #[must_use]
    pub fn resolve(peer: SocketAddr, headers: &HeaderMap, trusted: &TrustedProxies) -> Self {
        let peer_address = peer.ip();
        if !trusted.trusts(peer_address) {
            return Self {
                address: peer_address,
                scheme: Scheme::Http,
            };
        }

        // One walk, two facts. The chain says who the client is *and* how much
        // of every forwarded header the trusted edge wrote; deriving the
        // scheme from a second, independent rule would be two chances to
        // disagree about one chain (P7).
        let edge = Edge::resolve(headers, trusted);
        Self {
            address: edge.address.unwrap_or(peer_address),
            scheme: edge.scheme(headers),
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

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
    fn an_untrusted_peer_is_attributed_to_its_own_address() {
        let context = ClientContext::resolve(
            peer("203.0.113.9"),
            &headers(&[(FORWARDED_FOR, "1.2.3.4")]),
            &TrustedProxies::default(),
        );
        assert_eq!(context.address.to_string(), "203.0.113.9");
    }

    #[test]
    fn a_trusted_peer_has_its_forwarded_address_honoured() {
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[(FORWARDED_FOR, "1.2.3.4")]),
            &trusted,
        );
        assert_eq!(context.address.to_string(), "1.2.3.4");
    }

    #[test]
    fn the_hop_before_the_trusted_edge_wins_and_not_the_leftmost_entry() {
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[(FORWARDED_FOR, "1.2.3.4, 10.0.0.5")]),
            &trusted,
        );
        assert_eq!(context.address.to_string(), "1.2.3.4");
    }

    #[test]
    fn a_forged_leftmost_entry_does_not_choose_the_address_that_is_counted() {
        // The attack: the client sends `X-Forwarded-For: 9.9.9.9`, the trusted
        // proxy appends what it actually saw, and the header arrives as
        // "9.9.9.9, 203.0.113.9". Reading left to right lets the caller pick a
        // different identity on every request.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let attributed: Vec<String> = (0..5)
            .map(|n| {
                ClientContext::resolve(
                    peer("10.0.0.5"),
                    &headers(&[(FORWARDED_FOR, &format!("9.9.9.{n}, 203.0.113.9"))]),
                    &trusted,
                )
                .address
                .to_string()
            })
            .collect();
        assert_eq!(attributed, vec!["203.0.113.9"; 5]);
    }

    #[test]
    fn a_chain_of_trusted_hops_falls_back_to_the_client_most_of_them() {
        // Two proxies of the operator's own, and nothing beyond them: the
        // leftmost is as close to the client as the header goes.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[(FORWARDED_FOR, "10.0.0.9, 10.0.0.5")]),
            &trusted,
        );
        assert_eq!(context.address.to_string(), "10.0.0.9");
    }

    #[test]
    fn an_unreadable_entry_at_the_edge_falls_back_to_the_peer() {
        // "unknown" cannot be compared against the trusted list, so the chain
        // stops being provable there and the peer's own address is the answer.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[(FORWARDED_FOR, "1.2.3.4, unknown")]),
            &trusted,
        );
        assert_eq!(context.address.to_string(), "10.0.0.5");
    }

    #[test]
    fn a_trusted_peer_sending_nonsense_falls_back_to_its_own_address() {
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[(FORWARDED_FOR, "unknown")]),
            &trusted,
        );
        assert_eq!(context.address.to_string(), "10.0.0.5");
    }

    #[test]
    fn an_untrusted_peer_claiming_https_is_not_believed() {
        let context = ClientContext::resolve(
            peer("203.0.113.9"),
            &headers(&[(FORWARDED_PROTO, "https")]),
            &TrustedProxies::default(),
        );
        assert_eq!(context.scheme, Scheme::Http);
        assert!(!context.scheme.is_secure());
    }

    #[test]
    fn a_trusted_peer_claiming_https_is_believed() {
        let trusted = TrustedProxies::parse(&["10.0.0.5"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[(FORWARDED_PROTO, "HTTPS")]),
            &trusted,
        );
        assert_eq!(context.scheme, Scheme::Https);
    }

    #[test]
    fn a_forged_header_never_buys_a_second_identity() {
        // The shape of I-SEC-1's test: the same untrusted peer, a different
        // forged header each time, and one address to count against.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let attributed: Vec<String> = (0..5)
            .map(|n| {
                ClientContext::resolve(
                    peer("203.0.113.9"),
                    &headers(&[(FORWARDED_FOR, &format!("1.2.3.{n}"))]),
                    &trusted,
                )
                .address
                .to_string()
            })
            .collect();
        assert_eq!(attributed, vec!["203.0.113.9"; 5]);
    }
}
