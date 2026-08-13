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
