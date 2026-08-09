// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The instant every `_at` column stores.

use serde::{Deserialize, Serialize};

/// A point in time, held as milliseconds since the Unix epoch, UTC.
///
/// PRD §19.1 fixes this representation for every column whose name ends `_at`.
/// It is a newtype rather than a bare `i64` so a duration in milliseconds and
/// an instant in milliseconds cannot be swapped at a call site.
///
/// ```
/// use afisharr_core::time::Timestamp;
///
/// let t = Timestamp::from_millis(1_700_000_000_000);
/// assert_eq!(t.as_millis(), 1_700_000_000_000);
/// assert!(t.plus_millis(1) > t);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    /// The Unix epoch. Used by migration `0002` for rows that precede the instance.
    pub const EPOCH: Self = Self(0);

    /// Wraps a millisecond count that is already epoch-relative.
    #[must_use]
    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    /// The epoch-relative millisecond count, ready to bind to an `_at` column.
    #[must_use]
    pub const fn as_millis(self) -> i64 {
        self.0
    }

    /// This instant moved forward by `millis`, saturating instead of overflowing.
    #[must_use]
    pub const fn plus_millis(self, millis: i64) -> Self {
        Self(self.0.saturating_add(millis))
    }

    /// Milliseconds from `self` to `later`, saturating instead of overflowing.
    #[must_use]
    pub const fn millis_until(self, later: Self) -> i64 {
        later.0.saturating_sub(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plus_millis_saturates_at_the_upper_bound() {
        let far = Timestamp::from_millis(i64::MAX);
        assert_eq!(far.plus_millis(1), far);
    }

    #[test]
    fn millis_until_is_negative_when_the_argument_is_earlier() {
        let later = Timestamp::from_millis(10);
        let earlier = Timestamp::from_millis(4);
        assert_eq!(later.millis_until(earlier), -6);
    }

    #[test]
    fn ordering_follows_the_millisecond_count() {
        assert!(Timestamp::EPOCH < Timestamp::from_millis(1));
    }
}
