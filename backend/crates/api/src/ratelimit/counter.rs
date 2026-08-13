// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One counted window, and what is asked of it.
//!
//! Separated from the map that holds them because they are different concerns:
//! the limiter decides who is counted and when the map is swept, and this
//! decides what a single counter means. Every function here is total and takes
//! its `now` — none of them reads a clock, so each is answerable on its own.

use afisharr_core::time::Timestamp;

use crate::ratelimit::Policy;

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
pub(super) struct Counter {
    pub(super) window_started_at: Timestamp,
    pub(super) hits: u32,
    pub(super) locked_until: Option<Timestamp>,
    pub(super) consecutive_lockouts: u32,
}

impl Counter {
    /// A counter with its first window open at `now` and nothing counted yet.
    pub(super) const fn started(now: Timestamp) -> Self {
        Self {
            window_started_at: now,
            hits: 0,
            locked_until: None,
            consecutive_lockouts: 0,
        }
    }
}

/// Starts or escalates the lockout, returning how long the caller must wait.
pub(super) fn engage_lockout(counter: &mut Counter, policy: Policy, now: Timestamp) -> u64 {
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
pub(super) fn seconds_until(now: Timestamp, until: Timestamp) -> u64 {
    let millis = now.millis_until(until).max(1);
    u64::try_from((millis + 999) / 1000).unwrap_or(1).max(1)
}

/// When a counter stops being able to change any decision.
///
/// Not simply "when the window ends". A bucket that escalates carries its
/// consecutive-lockout count, and a counter dropped the moment its lockout
/// lifts hands a guesser the first rung of the ladder again on every wave —
/// which is the doubling PRD §21.4.3 asks for not existing. So a counter that
/// has ever been locked out is kept for the escalation's own ceiling past the
/// end of its last lockout, and an ordinary one only until its window closes.
pub(super) fn forgotten_at(counter: &Counter, policy: Policy) -> Timestamp {
    let window_ends = counter.window_started_at.plus_millis(policy.window_millis);
    let Some(lockout) = policy.lockout else {
        return window_ends;
    };
    if counter.consecutive_lockouts == 0 {
        return window_ends;
    }
    let ladder_ends = counter
        .locked_until
        .unwrap_or(counter.window_started_at)
        .plus_millis(lockout.ceiling_millis);
    window_ends.max(ladder_ends)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter(now: Timestamp) -> Counter {
        Counter {
            window_started_at: now,
            hits: 0,
            locked_until: None,
            consecutive_lockouts: 0,
        }
    }

    fn policy_with_lockout() -> Policy {
        crate::ratelimit::Bucket::login_account("operator").policy()
    }

    #[test]
    fn a_retry_time_is_rounded_up_and_is_never_zero() {
        let now = Timestamp::from_millis(1_000);
        assert_eq!(seconds_until(now, Timestamp::from_millis(1_001)), 1);
        assert_eq!(seconds_until(now, Timestamp::from_millis(2_500)), 2);
        // Already past: a caller told to wait zero seconds retries immediately
        // and is refused again, forever.
        assert_eq!(seconds_until(now, Timestamp::from_millis(0)), 1);
    }

    #[test]
    fn an_ordinary_counter_is_forgotten_when_its_window_closes() {
        let now = Timestamp::from_millis(1_000);
        let policy = crate::ratelimit::Bucket::Anonymous.policy();
        assert_eq!(
            forgotten_at(&counter(now), policy),
            now.plus_millis(policy.window_millis)
        );
    }

    #[test]
    fn a_counter_that_has_been_locked_out_outlives_its_window() {
        // The escalation ladder is the memory. Forgetting it the moment the
        // lockout lifts hands a guesser the first rung again on every wave.
        let now = Timestamp::from_millis(1_000);
        let policy = policy_with_lockout();
        let mut locked = counter(now);
        let waited = engage_lockout(&mut locked, policy, now);

        assert_eq!(waited, 15 * 60);
        assert!(
            forgotten_at(&locked, policy) > now.plus_millis(policy.window_millis),
            "a locked-out counter must outlive its own window"
        );
    }

    #[test]
    fn a_bucket_with_no_lockout_reports_the_rest_of_its_window() {
        let now = Timestamp::from_millis(1_000);
        let policy = crate::ratelimit::Bucket::Anonymous.policy();
        let mut spent = counter(now);
        assert_eq!(
            engage_lockout(&mut spent, policy, now),
            u64::try_from(policy.window_millis / 1000).expect("a minute fits"),
        );
        assert_eq!(spent.locked_until, None);
    }
}
