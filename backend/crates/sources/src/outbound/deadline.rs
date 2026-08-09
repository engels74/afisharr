// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The per-request deadline, which a call site may shorten and cannot omit.

use std::time::Duration;

/// A hard per-request timeout.
///
/// A newtype rather than a bare `Duration` because the constructor is the rule:
/// [`Deadline::shortened_to`] returns the tighter of the two, so a call site
/// that asks for five minutes against a client default of thirty seconds gets
/// thirty seconds. There is no constructor that lengthens one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Deadline(Duration);

impl Deadline {
    /// The client-wide default: thirty seconds.
    ///
    /// Long enough for a cold provider on an ordinary consumer WAN (PRD §21.1),
    /// short enough that a hung socket costs one request rather than a pass.
    pub const DEFAULT: Self = Self(Duration::from_secs(30));

    /// A deadline of `duration`.
    #[must_use]
    pub const fn of(duration: Duration) -> Self {
        Self(duration)
    }

    /// This deadline, or `duration` if that is tighter.
    ///
    /// Never the longer of the two: the client's deadline is a ceiling, and a
    /// call site that could raise it is a call site that can omit it.
    #[must_use]
    pub fn shortened_to(self, duration: Duration) -> Self {
        Self(self.0.min(duration))
    }

    /// The deadline as a `Duration`, ready to hand to the transport.
    #[must_use]
    pub const fn as_duration(self) -> Duration {
        self.0
    }
}

impl Default for Deadline {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_thirty_seconds() {
        assert_eq!(Deadline::default().as_duration(), Duration::from_secs(30));
    }

    #[test]
    fn a_tighter_request_deadline_wins() {
        let deadline = Deadline::DEFAULT.shortened_to(Duration::from_secs(2));
        assert_eq!(deadline.as_duration(), Duration::from_secs(2));
    }

    #[test]
    fn a_looser_request_deadline_does_not_raise_the_ceiling() {
        let deadline = Deadline::DEFAULT.shortened_to(Duration::from_mins(10));
        assert_eq!(deadline, Deadline::DEFAULT);
    }
}
