// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading the trusted chain, right to left, through the seam that decides it.
//!
//! Every case here drives [`ClientContext::resolve`], which is the one entry
//! point where `trustProxy`, the forwarded-header reader, and the right-to-left
//! walk meet. They lived inline in `proxy::edge` and tested none of it in
//! isolation: each asserts what the *whole* chain resolves a request to, so
//! they belong at the public seam rather than inside one of the four modules
//! that seam is built from.
//!
//! The header names are written out here rather than imported, because the
//! wire names are part of what an integration test is asserting.

use std::net::SocketAddr;

use afisharr_api::proxy::{ClientContext, Scheme, TrustedProxies};
use axum::http::{HeaderMap, HeaderValue};

const FORWARDED_FOR: &str = "x-forwarded-for";
const FORWARDED_PROTO: &str = "x-forwarded-proto";

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
