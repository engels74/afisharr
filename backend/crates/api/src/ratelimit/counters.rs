// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The swept map: what is filed under what, and when an entry stops mattering.

use std::{collections::HashMap, net::IpAddr};

use afisharr_core::time::Timestamp;

use crate::ratelimit::{
    Bucket,
    counter::{Counter, forgotten_at},
};

/// Every live counter, and what the last sweep left behind.
///
/// The sweep is the difference between a limiter and a leak. A counter is
/// created for every distinct thing counted — every address that reaches the
/// API, every account name a sign-in was attempted against — and none of those
/// is a value this instance chose. Resetting an expired window in place, as
/// `record` does, leaves the entry sitting in the map forever, so a caller who
/// varies the name they sign in with grows the process without bound and never
/// exceeds a single limit doing it.
#[derive(Debug)]
pub(super) struct Counters {
    pub(super) entries: HashMap<Key, Counter>,
    swept_at: Timestamp,
    /// How many counters survived the last sweep.
    ///
    /// The growth trigger waits for the map to double this, so a map that is
    /// entirely live costs an amortised constant per request instead of a full
    /// scan on every one.
    swept_len: usize,
}

/// How long a sweep waits before it is worth running again.
const SWEEP_INTERVAL_MILLIS: i64 = 60 * 1000;

/// How small the map may be before growth alone is not worth a sweep.
const SWEEP_FLOOR: usize = 1024;

/// What a counter is filed under: the bucket, and who is being counted.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Key {
    bucket: Bucket,
    address: Option<IpAddr>,
}

impl Key {
    /// The key one request is counted under.
    ///
    /// The address is dropped for the buckets that do not count per address,
    /// and it is dropped *here* rather than at the call sites. A caller that
    /// remembered to pass `None` and a caller that forgot would be two counters
    /// for one account, which is the failure this normalisation exists to make
    /// unreachable (P7).
    pub(super) fn of(bucket: &Bucket, address: Option<IpAddr>) -> Self {
        Self {
            bucket: bucket.clone(),
            address: if bucket.counts_per_address() {
                address
            } else {
                None
            },
        }
    }
}

impl Counters {
    /// An empty map, last swept at `now`.
    pub(super) fn new(now: Timestamp) -> Self {
        Self {
            entries: HashMap::new(),
            swept_at: now,
            swept_len: 0,
        }
    }

    /// Drops every counter that can no longer change a decision.
    ///
    /// Two triggers, and both are needed. Time alone leaves a burst between
    /// sweeps unbounded; growth alone never runs on a map that stopped
    /// growing. Growth is measured against what the last sweep left, so a map
    /// that is genuinely all live doubles before it is scanned again and the
    /// scan costs an amortised constant per request rather than a full pass on
    /// each one.
    pub(super) fn sweep(&mut self, now: Timestamp) {
        let overdue = self.swept_at.millis_until(now) >= SWEEP_INTERVAL_MILLIS;
        let grown = self.entries.len() >= self.swept_len.saturating_mul(2).max(SWEEP_FLOOR);
        if !overdue && !grown {
            return;
        }
        self.entries
            .retain(|key, counter| now < forgotten_at(counter, key.bucket.policy()));
        self.swept_at = now;
        self.swept_len = self.entries.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counters(now: Timestamp) -> Counters {
        Counters::new(now)
    }

    fn key(name: &str) -> Key {
        Key::of(&Bucket::login_account(name), None)
    }

    #[test]
    fn an_address_is_dropped_from_the_key_of_a_bucket_that_does_not_count_it() {
        // A caller that remembered to pass `None` and a caller that forgot
        // would be two counters for one account, which is the failure this
        // normalisation exists to make unreachable (P7).
        let with = Key::of(
            &Bucket::login_account("operator"),
            "1.2.3.4".parse::<IpAddr>().ok(),
        );
        assert_eq!(with, key("operator"));
    }

    #[test]
    fn a_bucket_that_counts_per_address_keeps_it() {
        let here = Key::of(&Bucket::Anonymous, "1.2.3.4".parse::<IpAddr>().ok());
        let there = Key::of(&Bucket::Anonymous, "9.9.9.9".parse::<IpAddr>().ok());
        assert_ne!(here, there);
    }

    #[test]
    fn a_sweep_that_is_neither_overdue_nor_provoked_by_growth_does_not_run() {
        // The scan is O(the map). Running it on every request would put a full
        // pass on the hot path of every answer this instance gives.
        let now = Timestamp::from_millis(1_000);
        let mut map = counters(now);
        map.entries.insert(key("operator"), Counter::started(now));

        map.sweep(now);
        assert_eq!(map.swept_len, 0, "an untriggered sweep leaves no trace");
        assert_eq!(map.entries.len(), 1);
    }

    #[test]
    fn growth_provokes_a_sweep_without_waiting_for_the_interval() {
        // Otherwise a burst between two sweeps is unbounded, which is the whole
        // of what an attacker controls.
        // Well past the fifteen-minute window every one of these opened in.
        let now = Timestamp::from_millis(60 * 60 * 1000);
        let mut map = counters(now);
        let stale = Counter::started(Timestamp::from_millis(0));
        for n in 0..=SWEEP_FLOOR {
            map.entries.insert(key(&format!("account-{n}")), stale);
        }

        map.sweep(now);
        assert!(
            map.entries.is_empty(),
            "{} stale counters survived",
            map.entries.len()
        );
    }
}
