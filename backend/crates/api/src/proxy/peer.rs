// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where a request really came from, and whether it really came over TLS.

use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;

use crate::proxy::{Claim, TrustedProxies, edge::Edge, trusted::canonical};

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
    /// Whether that scheme rests on the operator's `publicOrigin` alone.
    ///
    /// False for every scheme the chain itself stated, and true only where
    /// [`ClientContext::at_configured_origin`] read a request as HTTPS that no
    /// hop said anything about. The two are not interchangeable, and the
    /// difference decides `Strict-Transport-Security`: a `Secure` cookie set on
    /// a wrong reading is discarded by the browser and corrects itself the
    /// moment the deployment is fixed, while HSTS is remembered for a year and
    /// cannot be clicked through. So the inferred reading sets the cookie flag
    /// and never the header (`security::headers`).
    pub scheme_inferred: bool,
    /// What the forwarded chain said about the scheme.
    ///
    /// Carried as the resolved claim rather than as a position to re-read from,
    /// because [`ClientContext::at_configured_origin`] asks a second question of
    /// the same `X-Forwarded-Proto` chain — "did any hop state a scheme at
    /// all?" — and it must ask it of the value [`Self::scheme`] already came
    /// from. Two readers indexing the header for themselves is two chances to
    /// disagree about one chain (P7), and they did: a two-hop chain arriving as
    /// `http, https` answered "the hop stated TLS" there while the
    /// client-facing hop had said plaintext.
    pub(crate) stated: Claim,
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
        // Canonical from here on. A dual-stack listener reports an IPv4 peer as
        // `::ffff:a.b.c.d`, and carrying that spelling forward makes one caller
        // two rate-limit counters and two rows in the session list.
        let peer_address = canonical(peer.ip());
        if !trusted.trusts(peer_address) {
            return Self {
                address: peer_address,
                scheme: Scheme::Http,
                scheme_inferred: false,
                // Nothing here is honoured for the scheme — that is what the
                // `Http` above says — but the claim is still recorded, because
                // [`Self::at_configured_origin`] is written for exactly this
                // deployment: `trustProxy` empty, a real TLS proxy in front. It
                // is read at its weakest for the same reason, since no walk
                // proved anything about which hop wrote what.
                stated: Claim::of(headers, None),
            };
        }

        // One walk, two facts. The chain says who the client is *and* what it
        // claims about the scheme; deriving the scheme from a second,
        // independent rule would be two chances to disagree about one chain
        // (P7).
        let edge = Edge::resolve(headers, trusted);
        let stated = edge.claim(headers);
        Self {
            address: edge.address.unwrap_or(peer_address),
            scheme: if stated.is_tls() {
                Scheme::Https
            } else {
                Scheme::Http
            },
            scheme_inferred: false,
            stated,
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
    fn a_chain_of_nothing_but_trusted_hops_falls_back_to_the_peer() {
        // The walk reached the left end without finding a client, which is not
        // the same as the leftmost entry being one. A caller inside the trusted
        // range prepends whatever it likes and every entry passes, so reading
        // the leftmost hands it the address every limit is counted against.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[(FORWARDED_FOR, "10.0.0.9, 10.0.0.5")]),
            &trusted,
        );
        assert_eq!(context.address.to_string(), "10.0.0.5");
    }

    #[test]
    fn a_caller_inside_the_trusted_range_cannot_choose_what_it_is_counted_as() {
        // The containerised deployment this closes: `trustProxy` names the
        // bridge network, so a second container on it is trusted, and the edge
        // appends its address behind whatever it prepended. Rotating the
        // prepended value bought a fresh rate-limit counter every request.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let attributed: Vec<String> = (0..5)
            .map(|n| {
                ClientContext::resolve(
                    peer("10.0.0.5"),
                    &headers(&[(FORWARDED_FOR, &format!("198.51.100.{n}, 10.0.0.9"))]),
                    &trusted,
                )
                .address
                .to_string()
            })
            .collect();
        // 198.51.100.x is outside the trusted range, so the walk stops there
        // and the value is the client the edge really saw.
        assert_eq!(
            attributed,
            (0..5)
                .map(|n| format!("198.51.100.{n}"))
                .collect::<Vec<_>>()
        );

        let inside: Vec<String> = (0..5)
            .map(|n| {
                ClientContext::resolve(
                    peer("10.0.0.5"),
                    &headers(&[(FORWARDED_FOR, &format!("10.1.2.{n}, 10.0.0.9"))]),
                    &trusted,
                )
                .address
                .to_string()
            })
            .collect();
        assert_eq!(
            inside,
            vec!["10.0.0.5"; 5],
            "a prepended entry the trusted list happens to cover must not be believed"
        );
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
    fn a_dual_stack_listener_honours_the_operators_ipv4_trust_proxy() {
        // What `bind_address: "::"` reports for an IPv4 proxy. Compared raw,
        // the range matched nothing and the forwarded address was discarded, so
        // every client on the internet was counted as the proxy.
        let trusted = TrustedProxies::parse(&["172.16.0.0/12"]).expect("parses");
        let context = ClientContext::resolve(
            peer("[::ffff:172.18.0.2]"),
            &headers(&[(FORWARDED_FOR, "1.2.3.4"), (FORWARDED_PROTO, "https")]),
            &trusted,
        );
        assert_eq!(context.address.to_string(), "1.2.3.4");
        assert_eq!(context.scheme, Scheme::Https);
    }

    #[test]
    fn a_mapped_peer_is_recorded_under_one_spelling() {
        let context = ClientContext::resolve(
            peer("[::ffff:203.0.113.9]"),
            &HeaderMap::new(),
            &TrustedProxies::default(),
        );
        assert_eq!(context.address.to_string(), "203.0.113.9");
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
