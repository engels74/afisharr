// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where a request really came from, and whether it really came over TLS.

use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;

use crate::proxy::TrustedProxies;

/// `X-Forwarded-For`, honoured only from a trusted peer.
const FORWARDED_FOR: &str = "x-forwarded-for";

/// `X-Forwarded-Proto`, honoured only from a trusted peer.
const FORWARDED_PROTO: &str = "x-forwarded-proto";

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

        Self {
            address: forwarded_for(headers).unwrap_or(peer_address),
            scheme: forwarded_scheme(headers),
        }
    }
}

/// The client-most address in `X-Forwarded-For`.
///
/// The leftmost entry is the one the client claims, and the trusted proxy is
/// the one appending to it. Reading the leftmost is correct only because the
/// caller has already established that a trusted proxy built this header.
fn forwarded_for(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get(FORWARDED_FOR)?
        .to_str()
        .ok()?
        .split(',')
        .map(str::trim)
        .find_map(|entry| entry.parse::<IpAddr>().ok())
}

fn forwarded_scheme(headers: &HeaderMap) -> Scheme {
    let claimed = headers
        .get(FORWARDED_PROTO)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .unwrap_or_default();
    if claimed.eq_ignore_ascii_case("https") {
        Scheme::Https
    } else {
        Scheme::Http
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
    fn the_leftmost_forwarded_entry_wins_behind_a_trusted_proxy() {
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[(FORWARDED_FOR, "1.2.3.4, 10.0.0.5")]),
            &trusted,
        );
        assert_eq!(context.address.to_string(), "1.2.3.4");
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
