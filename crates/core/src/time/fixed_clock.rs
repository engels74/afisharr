// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A clock that moves only when it is told to.

use std::sync::atomic::{AtomicI64, Ordering};

use crate::time::{Clock, Timestamp};

/// A clock that stands still until [`FixedClock::advance`] is called.
///
/// Lease stealing, idle timeouts, and retention windows are all "what happens
/// when the clock passes this point" questions. Answering them with a sleep
/// makes a slow test; answering them with this makes an instant one.
///
/// ```
/// use afisharr_core::time::{Clock, FixedClock, Timestamp};
///
/// let clock = FixedClock::at(Timestamp::from_millis(1_000));
/// clock.advance(500);
/// assert_eq!(clock.now(), Timestamp::from_millis(1_500));
/// ```
#[derive(Debug)]
pub struct FixedClock(AtomicI64);

impl FixedClock {
    /// A clock stopped at `instant`.
    #[must_use]
    pub const fn at(instant: Timestamp) -> Self {
        Self(AtomicI64::new(instant.as_millis()))
    }

    /// Moves the clock forward by `millis`.
    pub fn advance(&self, millis: i64) {
        self.0.fetch_add(millis, Ordering::Relaxed);
    }

    /// Moves the clock to `instant`, forward or back.
    pub fn set(&self, instant: Timestamp) {
        self.0.store(instant.as_millis(), Ordering::Relaxed);
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_millis(self.0.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stands_still_between_advances() {
        let clock = FixedClock::at(Timestamp::from_millis(42));
        assert_eq!(clock.now(), clock.now());
        assert_eq!(clock.now(), Timestamp::from_millis(42));
    }

    #[test]
    fn set_moves_the_clock_backwards() {
        let clock = FixedClock::at(Timestamp::from_millis(42));
        clock.set(Timestamp::EPOCH);
        assert_eq!(clock.now(), Timestamp::EPOCH);
    }
}
