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
    #[must_use]
    pub fn trusts(&self, peer: IpAddr) -> bool {
        self.ranges.iter().any(|range| range.contains(&peer))
    }

    /// Whether any proxy is trusted at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
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
