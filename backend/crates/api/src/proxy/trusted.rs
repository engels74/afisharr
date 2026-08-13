// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The configured list of proxies whose forwarded headers count.

use std::net::IpAddr;

use ipnet::IpNet;
use thiserror::Error;

/// Why a `trustProxy` entry could not be read.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum TrustedProxyError {
    /// The entry is neither an IP address nor a CIDR range.
    #[error("'{0}' is not an IP address or CIDR range")]
    Malformed(String),
}

/// The addresses and ranges whose forwarded headers are honoured.
///
/// An empty list means no forwarded header is honoured anywhere, which is the
/// default and the safe direction (P2): the peer address is always available
/// and always true, and honouring a header nobody vouched for is the failure
/// `I-SEC-1` is written against.
#[derive(Debug, Clone, Default)]
pub struct TrustedProxies {
    ranges: Vec<IpNet>,
}

impl TrustedProxies {
    /// Reads the configured entries.
    ///
    /// A bare address is read as a single-host range, so the containment test
    /// below is one code path rather than two (P7).
    ///
    /// # Errors
    /// Returns [`TrustedProxyError::Malformed`] naming the entry that could
    /// not be read. Refused rather than skipped: an operator who mistyped a
    /// range and got a silently empty list would have a working proxy setup
    /// reporting every request from the proxy's own address.
    pub fn parse<S: AsRef<str>>(entries: &[S]) -> Result<Self, TrustedProxyError> {
        let mut ranges = Vec::with_capacity(entries.len());
        for entry in entries {
            let entry = entry.as_ref().trim();
            if entry.is_empty() {
                continue;
            }
            let range = entry.parse::<IpNet>().or_else(|_| {
                entry
                    .parse::<IpAddr>()
                    .map(IpNet::from)
                    .map_err(|_| TrustedProxyError::Malformed(entry.to_owned()))
            })?;
            ranges.push(range);
        }
        Ok(Self { ranges })
    }

    /// Whether `peer` is a proxy this instance takes forwarded headers from.
    ///
    /// The address is canonicalised first. See [`canonical`]: an IPv4 peer on a
    /// dual-stack listener arrives as `::ffff:a.b.c.d`, and an IPv4 range never
    /// contains a V6 address however the two are spelled.
    #[must_use]
    pub fn trusts(&self, peer: IpAddr) -> bool {
        let peer = canonical(peer);
        self.ranges.iter().any(|range| range.contains(&peer))
    }

    /// Whether any proxy is trusted at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }
}

/// The address as the family that owns it names it.
///
/// One rule, applied to every address this module compares or records, because
/// two spellings of one host are two of everything downstream (P7).
///
/// A dual-stack listener is what makes this necessary. `bind_address: "::"` is
/// how an instance serves IPv6, and it is what an IPv6-enabled Docker network
/// produces; Linux then accepts IPv4 connections on that socket and reports the
/// peer as `::ffff:172.18.0.2`. An IPv4 CIDR does not contain that address —
/// `IpNet::contains` compares families first — so `trustProxy: ["172.16.0.0/12"]`
/// matched nothing, every forwarded header was discarded, and every client on
/// the internet was counted and recorded as the proxy's one address, with
/// nothing anywhere saying the setting had not taken.
///
/// The same normalisation is owed to the addresses that *are* recorded. A client
/// resolved as `::ffff:1.2.3.4` on one request and `1.2.3.4` on the next is two
/// rate-limit counters and two rows in the operator's session list for one
/// caller.
#[must_use]
pub fn canonical(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(address, IpAddr::V4),
        IpAddr::V4(_) => address,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(text: &str) -> IpAddr {
        text.parse().expect("a valid address in the test")
    }

    #[test]
    fn the_default_trusts_nothing() {
        let proxies = TrustedProxies::default();
        assert!(proxies.is_empty());
        assert!(!proxies.trusts(address("127.0.0.1")));
    }

    #[test]
    fn a_bare_address_is_trusted_and_its_neighbours_are_not() {
        let proxies = TrustedProxies::parse(&["10.1.2.3"]).expect("parses");
        assert!(proxies.trusts(address("10.1.2.3")));
        assert!(!proxies.trusts(address("10.1.2.4")));
    }

    #[test]
    fn a_cidr_range_covers_its_members() {
        let proxies = TrustedProxies::parse(&["10.0.0.0/8"]).expect("parses");
        assert!(proxies.trusts(address("10.255.255.255")));
        assert!(!proxies.trusts(address("11.0.0.1")));
    }

    #[test]
    fn ipv6_ranges_work_the_same_way() {
        let proxies = TrustedProxies::parse(&["fd00::/8"]).expect("parses");
        assert!(proxies.trusts(address("fd00::1")));
        assert!(!proxies.trusts(address("fe80::1")));
    }

    #[test]
    fn an_ipv4_range_trusts_the_mapped_peer_a_dual_stack_socket_reports() {
        // `bind_address: "::"` accepts IPv4 connections and reports them as
        // `::ffff:a.b.c.d`. Compared raw, the operator's `trustProxy` matched
        // nothing: every forwarded header was discarded and every client behind
        // the proxy shared one address, one rate-limit counter, and one row in
        // the session list.
        let proxies = TrustedProxies::parse(&["172.16.0.0/12"]).expect("parses");
        assert!(proxies.trusts(address("::ffff:172.18.0.2")));
        assert!(!proxies.trusts(address("::ffff:192.0.2.1")));
    }

    #[test]
    fn canonicalising_leaves_every_other_address_alone() {
        assert_eq!(canonical(address("10.1.2.3")), address("10.1.2.3"));
        assert_eq!(canonical(address("2001:db8::1")), address("2001:db8::1"));
        assert_eq!(canonical(address("::ffff:10.1.2.3")), address("10.1.2.3"));
    }

    #[test]
    fn a_malformed_entry_is_refused_naming_itself() {
        let error = TrustedProxies::parse(&["10.0.0.0/8", "not-an-address"])
            .expect_err("a malformed entry must be refused");
        assert_eq!(
            error,
            TrustedProxyError::Malformed("not-an-address".to_owned())
        );
    }

    #[test]
    fn blank_entries_are_ignored_rather_than_refused() {
        // A compose file writing `TRUST_PROXY=10.0.0.0/8,` produces one, and it
        // means nothing rather than being a mistake.
        let proxies = TrustedProxies::parse(&["10.0.0.0/8", "  "]).expect("parses");
        assert!(proxies.trusts(address("10.1.1.1")));
    }
}
