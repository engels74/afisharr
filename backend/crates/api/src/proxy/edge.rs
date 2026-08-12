// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading the trusted chain, right to left.

use std::net::IpAddr;

use axum::http::HeaderMap;

use crate::proxy::{
    TrustedProxies,
    forwarded::{Claim, entries, parse_entry},
    peer::FORWARDED_FOR,
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
    pub(super) hops: usize,
    /// Whether the walk actually found the client, or gave up and said so.
    ///
    /// [`Self::hops`] is a position, and on an unprovable walk it is a floor
    /// rather than a measurement — one, the peer's own entry, because that is
    /// all this instance can attribute to anybody. Reading a *scheme* at a
    /// floor is not conservative: it picks the nearest hop's claim, which is
    /// the one entry an unprovable chain gives no reason to prefer. So the two
    /// facts are carried apart and [`Self::scheme`] reads each on its own
    /// terms.
    pub(super) proven: bool,
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
            proven: false,
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
                    proven: true,
                };
            }
        }
        // Every entry is a trusted address and the walk found no client, which
        // is not the same as the leftmost entry *being* the client. Returning
        // it was one hole: a caller inside the trusted range — a second
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
        // own address is the safe reading (P2).
        //
        // What this does *not* do, stated plainly because the neighbouring
        // return says `proven: true` and that word is easy to over-read: the
        // same caller prepending an address from *outside* the trusted range
        // still gets it back as the client, because the walk stops at the first
        // untrusted entry and that entry is the forged one. No walk can tell
        // that apart from a real proxy reporting a real client — reporting one
        // is precisely what a trusted proxy is trusted to do. `trustProxy` is
        // therefore the whole boundary: every host inside it can choose the
        // address this instance counts, logs, and stores, and an operator who
        // trusts a `/8` because their proxy lives in it has trusted every
        // container on that network with it. Naming the proxy rather than the
        // range it sits in is the only thing that narrows this, which is why
        // the cost of the narrow list — every client behind one proxy counted
        // as one when the walk ends here — is stated rather than hidden.
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
    ///
    /// An unprovable walk is read differently, and it has to be. There the hop
    /// count is a floor of one rather than a position, so indexing by it reads
    /// the *nearest* hop's entry — the reinstatement of the plaintext-edge
    /// upgrade this whole module removed for the provable case. An operator
    /// whose clients share a range with their proxy (`trustProxy: 10.0.0.0/8`,
    /// LAN clients in 10/8) has every walk end unprovable, so a browser reaching
    /// a plaintext edge that appends `http` and an internal hop that appends
    /// `https` was answered `Set-Cookie: …; Secure` — discarded by that browser,
    /// 401 on the next request, a sign-in loop — plus a year of HSTS on the
    /// apex and every subdomain, with nothing to click through.
    ///
    /// So an unprovable chain takes the weakest claim in it: TLS only when
    /// *every* entry says TLS ([`Claim::of`]). That keeps the two readings that
    /// were never in doubt — the single-entry chain a proxy overwrites, and an
    /// all-`https` chain — and refuses the upgrade the moment any hop in it says
    /// plaintext. It cannot be forged upward either: a caller can prepend
    /// entries, and prepending anything but `https` only weakens the answer.
    pub(super) fn claim(&self, headers: &HeaderMap) -> Claim {
        Claim::of(headers, self.proven.then_some(self.hops))
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use axum::http::{HeaderMap, HeaderValue};

    use super::FORWARDED_FOR;
    use crate::proxy::peer::FORWARDED_PROTO;
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
    fn an_all_trusted_chain_does_not_upgrade_on_the_nearest_hops_entry() {
        // The deployment: `trustProxy` names the range the operator's own LAN
        // clients sit in, so every walk ends unprovable. A browser reaches an
        // edge still listening on plain `:80`, which appends `http`, and an
        // internal hop re-encrypts and appends `https`. Reading the rightmost
        // entry answered TLS, so the sign-in set `Secure` on a cookie that
        // browser discards — 401 on the next request, a login loop — and HSTS
        // pinned the name and every subdomain for a year.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        let context = ClientContext::resolve(
            peer("10.0.0.5"),
            &headers(&[
                (FORWARDED_FOR, "10.1.2.3, 10.0.0.9"),
                (FORWARDED_PROTO, "http, https"),
            ]),
            &trusted,
        );
        assert_eq!(context.address.to_string(), "10.0.0.5");
        assert_eq!(context.scheme, Scheme::Http);
    }

    #[test]
    fn an_all_trusted_chain_that_says_tls_throughout_is_still_read_as_tls() {
        // The bound on the rule above. The single-entry chain a proxy overwrites
        // is the overwhelmingly common configuration, and refusing it would
        // strip `Secure` from every cookie on a working TLS deployment.
        let trusted = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        for chain in ["https", "https, https"] {
            let context = ClientContext::resolve(
                peer("10.0.0.5"),
                &headers(&[
                    (FORWARDED_FOR, "10.1.2.3, 10.0.0.9"),
                    (FORWARDED_PROTO, chain),
                ]),
                &trusted,
            );
            assert_eq!(context.scheme, Scheme::Https, "chain {chain:?}");
        }
    }

    #[test]
    fn a_chain_that_cannot_be_walked_reads_the_scheme_from_the_peer_s_own_entry() {
        // "unknown" ends the walk, so nothing in the chain is attributable and
        // the weakest claim in it is the answer: one entry says plaintext.
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
