// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading a forwarded request as HTTPS because of where it arrived.
//!
//! The one fact about a request's scheme that does not come from the forwarded
//! chain. It is kept apart from [`ClientContext::resolve`] for that reason: the
//! chain is evidence about this request, and `publicOrigin` is the operator's
//! statement about the instance, and the second is only ever read where the
//! first left the question open.

use axum::http::{HeaderMap, header::HOST};

use crate::proxy::{
    ClientContext, PublicOrigin, Scheme, edge,
    peer::{FORWARDED_FOR, FORWARDED_PROTO},
};

impl ClientContext {
    /// Reads a forwarded request as HTTPS when it arrived at the `https` address
    /// the operator configured.
    ///
    /// The gap this closes: `trustProxy` is empty by default, so a stock
    /// deployment behind Caddy, nginx, or Cloudflare on an HTTPS address
    /// discards the proxy's `X-Forwarded-Proto` and resolves as plaintext. The
    /// session cookie is then set without `Secure` on a connection that is
    /// carrying TLS, and nothing anywhere says the instance is in that state.
    ///
    /// The operator's `publicOrigin` is what makes an answer possible, because
    /// it is a statement about this instance rather than about one request. Read
    /// together with the `Host` the browser wrote from the URL it is calling, it
    /// says the request arrived at the address the operator declared to be
    /// HTTPS.
    ///
    /// `Host` alone is not enough, and reading it that way was worse than the
    /// gap. It is written by whoever is calling, so an instance also reachable
    /// over plaintext at that same name — a proxy listening on `:80` that
    /// forwards without redirecting, split-horizon DNS to the LAN — answered a
    /// sign-in on the plaintext connection with `Set-Cookie: …; Secure`. The
    /// browser discarded the cookie, the next request was 401, and the sign-in
    /// both succeeded and did not. The same answer carried
    /// `Strict-Transport-Security`, pinning every subdomain of that name for a
    /// year, from a header the caller wrote.
    ///
    /// So the `Host` decides nothing on its own. It is read only when something
    /// in front of this instance forwarded the request, and never when a hop
    /// says it served the client over plaintext:
    ///
    /// - `X-Forwarded-Proto: https` — the stock TLS proxy. Upgraded.
    /// - `X-Forwarded-For` with no proto — a hop that says nothing about the
    ///   scheme. The configured origin is the only evidence there is, and it is
    ///   what this exists for. Upgraded.
    /// - `X-Forwarded-Proto: http` — the hop states plaintext. Left alone.
    /// - No forwarded header at all — nothing forwarded this, so it arrived here
    ///   over the plaintext this instance serves. Left alone.
    ///
    /// The nearest hop's entry is the one read, for the reason
    /// [`Edge::scheme`] indexes from the right: the leftmost entry is whatever
    /// the caller prepended. A caller reaching this instance directly can still
    /// write the whole header, and gains a `Secure` cookie their own plaintext
    /// browser discards and HSTS in their own browser — nobody else's.
    ///
    /// It does not cover every deployment. A proxy that rewrites `Host` to the
    /// upstream's own name — `proxy_pass` with no `proxy_set_header Host` —
    /// leaves nothing here to match, and that instance still has to name its
    /// proxy in `trustProxy` for the forwarded scheme to be honoured at all.
    #[must_use]
    pub fn at_configured_origin(
        mut self,
        headers: &HeaderMap,
        configured: Option<&PublicOrigin>,
    ) -> Self {
        if self.scheme.is_secure() || !crossed_a_proxy(headers) || claims_plaintext(headers) {
            return self;
        }
        let Some(origin) = configured.filter(|origin| origin.is_secure()) else {
            return self;
        };
        let arrived_at_it = headers
            .get(HOST)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|host| origin.matches_host(host));
        if arrived_at_it {
            self.scheme = Scheme::Https;
        }
        self
    }
}

/// Whether anything in front of this instance forwarded this request.
fn crossed_a_proxy(headers: &HeaderMap) -> bool {
    headers.contains_key(FORWARDED_FOR) || headers.contains_key(FORWARDED_PROTO)
}

/// Whether the nearest hop states it served the client over plaintext.
///
/// Absent is not a claim: a proxy that forwards `X-Forwarded-For` and leaves the
/// scheme alone has said nothing, and this answers `false` for it.
fn claims_plaintext(headers: &HeaderMap) -> bool {
    edge::entries(headers, FORWARDED_PROTO)
        .last()
        .is_some_and(|claimed| !claimed.eq_ignore_ascii_case("https"))
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use axum::http::HeaderValue;

    use super::*;
    use crate::proxy::TrustedProxies;

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

    /// The stock deployment this whole upgrade exists for: TLS terminates at a
    /// proxy, and `trustProxy` is empty, so nothing forwarded is honoured.
    fn resolved_at(entries: &[(&'static str, &str)], configured: &str) -> Scheme {
        let origin = PublicOrigin::parse(configured).expect("a valid origin");
        ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(entries),
            &TrustedProxies::default(),
        )
        .at_configured_origin(&headers(entries), Some(&origin))
        .scheme
    }

    #[test]
    fn a_proxy_that_reports_tls_at_the_configured_origin_is_read_as_secure() {
        assert_eq!(
            resolved_at(
                &[
                    ("host", "media.example"),
                    (FORWARDED_FOR, "1.2.3.4"),
                    (FORWARDED_PROTO, "https"),
                ],
                "https://media.example",
            ),
            Scheme::Https
        );
    }

    #[test]
    fn a_proxy_that_reports_nothing_at_the_configured_origin_is_read_as_secure() {
        // The gap the configured origin closes: a hop that forwards the address
        // and leaves the scheme alone. Without this the session cookie is set
        // without `Secure` on a connection that is carrying TLS.
        assert_eq!(
            resolved_at(
                &[("host", "media.example"), (FORWARDED_FOR, "1.2.3.4")],
                "https://media.example",
            ),
            Scheme::Https
        );
    }

    #[test]
    fn a_hop_that_states_plaintext_is_not_overruled_by_the_host() {
        // A proxy listening on `:80` that forwards without redirecting. Read as
        // HTTPS, the sign-in answers `Set-Cookie: …; Secure` over plaintext:
        // the browser discards it, the next request is 401, and the answer also
        // carries HSTS for every subdomain of that name.
        assert_eq!(
            resolved_at(
                &[
                    ("host", "media.example"),
                    (FORWARDED_FOR, "1.2.3.4"),
                    (FORWARDED_PROTO, "http"),
                ],
                "https://media.example",
            ),
            Scheme::Http
        );
    }

    #[test]
    fn a_request_nothing_forwarded_is_not_upgraded_by_its_host_header() {
        // Split-horizon DNS, or the LAN address, reaching this instance
        // directly over the plaintext it serves. `Host` is written by whoever
        // is calling, so on its own it is not evidence of anything.
        assert_eq!(
            resolved_at(&[("host", "media.example")], "https://media.example"),
            Scheme::Http
        );
    }

    #[test]
    fn another_host_is_not_the_configured_origin() {
        // Whatever else the request carries. The upgrade rests on the operator's
        // statement about this instance, and a name they did not configure is
        // not that statement.
        for entries in [
            &[("host", "evil.example"), (FORWARDED_FOR, "1.2.3.4")][..],
            &[
                ("host", "evil.example"),
                (FORWARDED_FOR, "1.2.3.4"),
                (FORWARDED_PROTO, "https"),
            ][..],
        ] {
            assert_eq!(
                resolved_at(entries, "https://media.example"),
                Scheme::Http,
                "{entries:?}"
            );
        }
    }

    #[test]
    fn a_plaintext_configured_origin_upgrades_nothing() {
        assert_eq!(
            resolved_at(
                &[("host", "media.example"), (FORWARDED_FOR, "1.2.3.4")],
                "http://media.example",
            ),
            Scheme::Http
        );
    }
}
