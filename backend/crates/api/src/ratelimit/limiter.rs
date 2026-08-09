// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The counters themselves.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, RwLock},
};

use afisharr_core::time::{Clock, Timestamp};

use crate::ratelimit::{Bucket, Policy};

/// What the limiter decided about one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Inside the allowance. Proceed.
    Allowed,
    /// Over the allowance, or locked out.
    Refused {
        /// How long the caller must wait, in seconds, rounded up.
        retry_after_seconds: u64,
    },
}

/// One counted window, plus how many consecutive lockouts it has earned.
#[derive(Debug, Clone, Copy)]
struct Counter {
    window_started_at: Timestamp,
    hits: u32,
    locked_until: Option<Timestamp>,
    consecutive_lockouts: u32,
}

/// The in-memory limiter.
///
/// In memory and not in the database, deliberately: this is a per-process
/// counter on the hot path of every request, and D-024's single write actor
/// exists so that mutations are serialised — putting a counter increment
/// through it would serialise reads behind writes for no correctness gain. A
/// restart clears the counters, which costs an attacker one restart they do
/// not control and costs a locked-out operator nothing they did not already
/// have through the console.
///
/// An `RwLock` over a `HashMap` rather than a `Mutex`: the common path is a
/// bucket that already exists, and the write is short and uncontended at four
/// concurrent operators (PRD §21.1).
#[derive(Debug, Clone)]
pub struct RateLimiter {
    counters: Arc<RwLock<HashMap<Key, Counter>>>,
    clock: Arc<dyn Clock>,
}

/// What a counter is filed under: the bucket, and who is being counted.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    bucket: Bucket,
    address: Option<IpAddr>,
}

impl RateLimiter {
    /// An empty limiter reading `clock`.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            clock,
        }
    }

    /// Whether `address` may make one more request in `bucket`, without
    /// counting it.
    ///
    /// Used by the buckets that count failures only: a sign-in has to be
    /// allowed to run before anyone knows whether it failed.
    #[must_use]
    pub fn check(&self, bucket: &Bucket, address: Option<IpAddr>) -> Decision {
        let now = self.clock.now();
        let key = Key {
            bucket: bucket.clone(),
            address,
        };
        let policy = bucket.policy();
        let counters = self
            .counters
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        counters
            .get(&key)
            .map_or(Decision::Allowed, |counter| judge(counter, policy, now))
    }

    /// Counts one request against `bucket` and reports whether it is allowed.
    #[must_use]
    pub fn record(&self, bucket: &Bucket, address: Option<IpAddr>) -> Decision {
        let now = self.clock.now();
        let policy = bucket.policy();
        let key = Key {
            bucket: bucket.clone(),
            address,
        };

        let mut counters = self
            .counters
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let counter = counters.entry(key).or_insert(Counter {
            window_started_at: now,
            hits: 0,
            locked_until: None,
            consecutive_lockouts: 0,
        });

        // A live lockout refuses without counting: an attempt made during a
        // lockout must not extend it, or a client that retries in a loop locks
        // an operator out for the ceiling and never lets go.
        if let Some(until) = counter.locked_until {
            if now < until {
                return Decision::Refused {
                    retry_after_seconds: seconds_until(now, until),
                };
            }
            counter.locked_until = None;
            counter.window_started_at = now;
            counter.hits = 0;
        }

        if counter.window_started_at.millis_until(now) >= policy.window_millis {
            counter.window_started_at = now;
            counter.hits = 0;
        }

        counter.hits += 1;
        if counter.hits > policy.allowance {
            return Decision::Refused {
                retry_after_seconds: engage_lockout(counter, policy, now),
            };
        }
        Decision::Allowed
    }

    /// Clears the counter for one bucket.
    ///
    /// A successful sign-in runs this: the failure count exists to slow a
    /// guesser down, and the operator who just proved who they are is not one.
    pub fn forget(&self, bucket: &Bucket, address: Option<IpAddr>) {
        let key = Key {
            bucket: bucket.clone(),
            address,
        };
        let mut counters = self
            .counters
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        counters.remove(&key);
    }
}

/// Reads a counter without changing it.
fn judge(counter: &Counter, policy: Policy, now: Timestamp) -> Decision {
    if let Some(locked_until) = counter.locked_until
        && now < locked_until
    {
        return Decision::Refused {
            retry_after_seconds: seconds_until(now, locked_until),
        };
    }
    if counter.window_started_at.millis_until(now) >= policy.window_millis {
        return Decision::Allowed;
    }
    if counter.hits >= policy.allowance {
        let window_ends = counter.window_started_at.plus_millis(policy.window_millis);
        return Decision::Refused {
            retry_after_seconds: seconds_until(now, window_ends),
        };
    }
    Decision::Allowed
}

/// Starts or escalates the lockout, returning how long the caller must wait.
fn engage_lockout(counter: &mut Counter, policy: Policy, now: Timestamp) -> u64 {
    let Some(lockout) = policy.lockout else {
        let window_ends = counter.window_started_at.plus_millis(policy.window_millis);
        return seconds_until(now, window_ends);
    };
    counter.consecutive_lockouts = counter.consecutive_lockouts.saturating_add(1);
    let until = now.plus_millis(lockout.duration_millis(counter.consecutive_lockouts));
    counter.locked_until = Some(until);
    seconds_until(now, until)
}

/// Whole seconds from `now` to `until`, rounded up, and never zero.
fn seconds_until(now: Timestamp, until: Timestamp) -> u64 {
    let millis = now.millis_until(until).max(1);
    u64::try_from((millis + 999) / 1000).unwrap_or(1).max(1)
}

#[cfg(test)]
mod tests {
    use afisharr_core::time::FixedClock;

    use super::*;

    fn limiter() -> (Arc<FixedClock>, RateLimiter) {
        let clock = Arc::new(FixedClock::at(Timestamp::from_millis(1_700_000_000_000)));
        let limiter = RateLimiter::new(clock.clone());
        (clock, limiter)
    }

    fn address(text: &str) -> Option<IpAddr> {
        text.parse().ok()
    }

    #[test]
    fn requests_inside_the_allowance_are_allowed() {
        let (_clock, limiter) = limiter();
        for _ in 0..20 {
            assert_eq!(
                limiter.record(&Bucket::LoginAddress, address("1.2.3.4")),
                Decision::Allowed
            );
        }
    }

    #[test]
    fn the_request_after_the_allowance_is_refused_with_a_retry_time() {
        let (_clock, limiter) = limiter();
        for _ in 0..20 {
            let _ = limiter.record(&Bucket::LoginAddress, address("1.2.3.4"));
        }
        let Decision::Refused {
            retry_after_seconds,
        } = limiter.record(&Bucket::LoginAddress, address("1.2.3.4"))
        else {
            panic!("the twenty-first attempt must be refused");
        };
        assert!(retry_after_seconds > 0);
        assert!(retry_after_seconds <= 15 * 60);
    }

    #[test]
    fn two_addresses_are_counted_separately() {
        let (_clock, limiter) = limiter();
        for _ in 0..21 {
            let _ = limiter.record(&Bucket::LoginAddress, address("1.2.3.4"));
        }
        assert_eq!(
            limiter.record(&Bucket::LoginAddress, address("5.6.7.8")),
            Decision::Allowed
        );
    }

    #[test]
    fn the_window_lapses_and_the_allowance_returns() {
        let (clock, limiter) = limiter();
        for _ in 0..21 {
            let _ = limiter.record(&Bucket::Api, address("1.2.3.4"));
        }
        clock.advance(60 * 1000);
        assert_eq!(
            limiter.record(&Bucket::Api, address("1.2.3.4")),
            Decision::Allowed
        );
    }

    #[test]
    fn an_account_lockout_escalates_across_consecutive_lockouts() {
        let (clock, limiter) = limiter();
        let bucket = Bucket::LoginAccount {
            username: "operator".to_owned(),
        };

        let first = spend_to_lockout(&limiter, &bucket);
        assert_eq!(first, 15 * 60);

        clock.advance(15 * 60 * 1000);
        let second = spend_to_lockout(&limiter, &bucket);
        assert_eq!(second, 30 * 60);

        clock.advance(30 * 60 * 1000);
        let third = spend_to_lockout(&limiter, &bucket);
        assert_eq!(third, 60 * 60);
    }

    #[test]
    fn a_successful_sign_in_clears_the_failure_count() {
        let (_clock, limiter) = limiter();
        let bucket = Bucket::LoginAccount {
            username: "operator".to_owned(),
        };
        for _ in 0..5 {
            let _ = limiter.record(&bucket, address("1.2.3.4"));
        }
        limiter.forget(&bucket, address("1.2.3.4"));
        assert_eq!(
            limiter.check(&bucket, address("1.2.3.4")),
            Decision::Allowed
        );
    }

    #[test]
    fn check_reports_the_refusal_without_spending_an_attempt() {
        let (_clock, limiter) = limiter();
        for _ in 0..5 {
            let _ = limiter.record(&Bucket::SetupAttempt, address("1.2.3.4"));
        }
        // Five spent; the sixth is over. `check` must say so twice without the
        // second call making it worse.
        let first = limiter.check(&Bucket::SetupAttempt, address("1.2.3.4"));
        let second = limiter.check(&Bucket::SetupAttempt, address("1.2.3.4"));
        assert!(matches!(first, Decision::Refused { .. }));
        assert_eq!(first, second);
    }

    fn spend_to_lockout(limiter: &RateLimiter, bucket: &Bucket) -> u64 {
        let mut last = Decision::Allowed;
        for _ in 0..6 {
            last = limiter.record(bucket, address("1.2.3.4"));
        }
        match last {
            Decision::Refused {
                retry_after_seconds,
            } => retry_after_seconds,
            Decision::Allowed => panic!("the sixth failure must lock the account out"),
        }
    }
}
