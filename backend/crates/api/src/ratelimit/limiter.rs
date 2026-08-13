// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The limiter itself: one lock, two questions.
//!
//! Both questions change the counter, and that is deliberate. There was a third
//! — "is this bucket spent?", asked without spending it — and the sign-in path
//! was built on it: check the allowance, run the password hash, count the
//! failure afterwards. A check that changes nothing cannot bound work that has
//! not finished, so every request in a burst that arrived before the first hash
//! completed read the same empty counter and was let through together. Twenty
//! guesses against a five-guess account all ran, and the counters recorded them
//! honestly a quarter of a second later, by which time the answer was known.
//!
//! So a caller takes its attempt *before* the work, and hands the whole counter
//! back with [`RateLimiter::forget`] when the attempt turns out not to have been
//! one worth counting — which for a sign-in means it succeeded.

use std::{
    net::IpAddr,
    sync::{Arc, RwLock},
};

use afisharr_core::time::Clock;

use crate::ratelimit::{
    Bucket, Decision,
    counter::{Counter, engage_lockout, seconds_until},
    counters::{Counters, Key},
};

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
    counters: Arc<RwLock<Counters>>,
    clock: Arc<dyn Clock>,
}

impl RateLimiter {
    /// An empty limiter reading `clock`.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        let swept_at = clock.now();
        Self {
            counters: Arc::new(RwLock::new(Counters::new(swept_at))),
            clock,
        }
    }

    /// Takes one attempt from `bucket` and reports whether it was there to
    /// take.
    ///
    /// The increment and the judgement happen under one write lock, which is
    /// what makes the count a bound rather than a report: two callers cannot
    /// both read the last remaining attempt.
    #[must_use]
    pub fn record(&self, bucket: &Bucket, address: Option<IpAddr>) -> Decision {
        let now = self.clock.now();
        let policy = bucket.policy();
        let key = Key::of(bucket, address);

        let mut counters = self
            .counters
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Before the insert, not after: the entry about to be added is live by
        // definition, and sweeping first is what keeps the map's size a
        // function of what is currently being limited rather than of
        // everything that ever was.
        counters.sweep(now);
        let counter = counters
            .entries
            .entry(key)
            .or_insert_with(|| Counter::started(now));

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
        // One rule for every bucket: the allowance is what the window permits,
        // and the request that would exceed it is the one refused. Because this
        // is the gate and not a report written afterwards, the refusal lands on
        // the caller it describes — the sixth guess against a five-guess
        // account is refused here, and it is here that the escalating lockout
        // engages, once per wave rather than once per request.
        if counter.hits > policy.allowance {
            return Decision::Refused {
                retry_after_seconds: engage_lockout(counter, policy, now),
            };
        }
        Decision::Allowed
    }

    /// How many counters are held.
    ///
    /// The failure this exists to catch is a size and not a behaviour: a map
    /// that keeps every account name it has ever been asked about answers every
    /// question correctly right up until the process runs out of memory.
    #[cfg(test)]
    fn held(&self) -> usize {
        self.counters
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .len()
    }

    /// Clears the counter for one bucket.
    ///
    /// A successful sign-in runs this, and it is the other half of taking the
    /// attempt up front: the count exists to slow a guesser down, the operator
    /// who just proved who they are is not one, and the attempt they reserved
    /// on the way in goes back with everything else in the bucket.
    pub fn forget(&self, bucket: &Bucket, address: Option<IpAddr>) {
        let key = Key::of(bucket, address);
        let mut counters = self
            .counters
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        counters.entries.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use afisharr_core::time::{FixedClock, Timestamp};

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
    fn one_account_is_one_counter_however_many_addresses_try_it() {
        // The attack this closes: rotate the source address and get the whole
        // allowance again against the same account, five attempts at a time,
        // for as many addresses as the attacker can reach the instance from.
        let (_clock, limiter) = limiter();
        let bucket = Bucket::LoginAccount {
            username: "operator".to_owned(),
        };
        for n in 0..5 {
            let _ = limiter.record(&bucket, address(&format!("203.0.113.{n}")));
        }
        assert!(matches!(
            limiter.record(&bucket, address("203.0.113.99")),
            Decision::Refused { .. }
        ));
    }

    #[test]
    fn the_allowance_bounds_the_attempts_that_run_rather_than_the_failures_counted() {
        // The bypass this closes. Every one of these calls stands for a request
        // that arrived before any of the others had finished its password hash,
        // so under a limiter consulted read-only they all read an empty counter
        // and all ran. Here the counter is taken, not read, so exactly the
        // allowance gets through however tightly they arrive.
        let (_clock, limiter) = limiter();
        let bucket = Bucket::login_account("operator");
        let allowed = (0..20)
            .filter(|_| limiter.record(&bucket, address("1.2.3.4")) == Decision::Allowed)
            .count();
        assert_eq!(allowed, 5, "a burst must not get more than the allowance");
    }

    #[test]
    fn the_lockout_engages_on_the_attempt_that_exceeds_the_allowance() {
        let (_clock, limiter) = limiter();
        let bucket = Bucket::LoginAccount {
            username: "operator".to_owned(),
        };
        for _ in 0..5 {
            assert_eq!(
                limiter.record(&bucket, address("1.2.3.4")),
                Decision::Allowed
            );
        }
        let Decision::Refused {
            retry_after_seconds,
        } = limiter.record(&bucket, address("1.2.3.4"))
        else {
            panic!("the sixth attempt must be refused");
        };
        assert_eq!(retry_after_seconds, 15 * 60);
    }

    #[test]
    fn the_window_lapses_and_the_allowance_returns() {
        let (clock, limiter) = limiter();
        for _ in 0..21 {
            let _ = limiter.record(&Bucket::Anonymous, address("1.2.3.4"));
        }
        clock.advance(60 * 1000);
        assert_eq!(
            limiter.record(&Bucket::Anonymous, address("1.2.3.4")),
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
    fn a_successful_sign_in_clears_the_attempts_it_took() {
        let (_clock, limiter) = limiter();
        let bucket = Bucket::LoginAccount {
            username: "operator".to_owned(),
        };
        for _ in 0..5 {
            let _ = limiter.record(&bucket, address("1.2.3.4"));
        }
        limiter.forget(&bucket, address("1.2.3.4"));
        assert_eq!(
            limiter.record(&bucket, address("1.2.3.4")),
            Decision::Allowed
        );
    }

    fn spend_to_lockout(limiter: &RateLimiter, bucket: &Bucket) -> u64 {
        let mut last = Decision::Allowed;
        // One past the allowance: the attempt that exceeds it is the one the
        // lockout engages on.
        for _ in 0..6 {
            last = limiter.record(bucket, address("1.2.3.4"));
        }
        match last {
            Decision::Refused {
                retry_after_seconds,
            } => retry_after_seconds,
            Decision::Allowed => panic!("spending the allowance must lock the account out"),
        }
    }

    #[test]
    fn a_counter_whose_window_has_closed_is_dropped_rather_than_reset() {
        // The leak this closes: every distinct account name a sign-in is tried
        // against creates an entry, `record` resets the expired window in
        // place, and nothing ever removes it. A caller who varies the name
        // grows the process for as long as it runs, without exceeding a single
        // limit while doing it.
        let (clock, limiter) = limiter();
        for n in 0..64 {
            let _ = limiter.record(&Bucket::login_account(&format!("account-{n}")), None);
        }
        assert_eq!(limiter.held(), 64);

        clock.advance(15 * 60 * 1000 + 1);
        let _ = limiter.record(&Bucket::login_account("account-fresh"), None);

        assert_eq!(
            limiter.held(),
            1,
            "a window nobody can be refused by must not survive"
        );
    }

    #[test]
    fn a_counter_that_can_still_refuse_survives_the_sweep() {
        let (clock, limiter) = limiter();
        for _ in 0..5 {
            let _ = limiter.record(&Bucket::SetupAttempt, address("1.2.3.4"));
        }

        // Past the sweep interval, and nowhere near the fifteen-minute window.
        clock.advance(61_000);
        let _ = limiter.record(&Bucket::SetupAttempt, address("9.9.9.9"));

        assert!(
            matches!(
                limiter.record(&Bucket::SetupAttempt, address("1.2.3.4")),
                Decision::Refused { .. }
            ),
            "the sweep must not hand a spent allowance back"
        );
    }

    #[test]
    fn a_lockout_that_has_lifted_still_remembers_the_rung_it_reached() {
        let (clock, limiter) = limiter();
        let bucket = Bucket::login_account("operator");
        assert_eq!(spend_to_lockout(&limiter, &bucket), 15 * 60);

        clock.advance(15 * 60 * 1000 + 1);
        for n in 0..8 {
            let _ = limiter.record(&Bucket::login_account(&format!("noise-{n}")), None);
        }

        assert_eq!(
            spend_to_lockout(&limiter, &bucket),
            30 * 60,
            "the escalation must survive the sweep"
        );
    }
}
