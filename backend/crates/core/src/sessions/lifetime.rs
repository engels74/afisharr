// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The two timeouts that bound a session, and how a stored row is judged.

use crate::time::Timestamp;

/// Seven days, sliding on `last_seen_at` (PRD §21.4.2).
pub const IDLE_TIMEOUT_MILLIS: i64 = 7 * 24 * 60 * 60 * 1000;

/// Thirty days from creation, with no extension (PRD §21.4.2).
pub const ABSOLUTE_LIFETIME_MILLIS: i64 = 30 * 24 * 60 * 60 * 1000;

/// One row of `sessions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// SHA-256 of the cookie value.
    pub digest: String,
    /// The account this session signs in as.
    pub user_id: String,
    /// When the session was minted.
    pub created_at: Timestamp,
    /// The absolute deadline, thirty days from creation.
    pub expires_at: Timestamp,
    /// The instant of the last request that carried this session.
    pub last_seen_at: Timestamp,
    /// The user agent recorded at creation, for the sessions list in Settings.
    pub user_agent: Option<String>,
    /// The peer address recorded at creation.
    pub ip: Option<String>,
    /// When the session was revoked, if it was.
    pub revoked_at: Option<Timestamp>,
}

/// Why a stored session is or is not usable right now.
///
/// Four values rather than a bool, because the interface says something
/// different for each and the audit trail wants the distinction: a revoked
/// session was taken away, an idle one lapsed on its own, and an expired one
/// hit a ceiling that no amount of activity moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validity {
    /// Usable, and the idle window should be slid forward.
    Active,
    /// Revoked explicitly, by a sign-out or a password change.
    Revoked,
    /// Nothing carried this session inside the idle window.
    Idle,
    /// The thirty-day ceiling has passed.
    Expired,
}

impl Session {
    /// Judges this session at `now`.
    ///
    /// Revocation is checked first, then the absolute ceiling, then idleness.
    /// The order is what makes the reason reported the most specific one true:
    /// a session revoked forty days ago is revoked, not expired.
    #[must_use]
    pub fn validity(&self, now: Timestamp) -> Validity {
        if self.revoked_at.is_some() {
            return Validity::Revoked;
        }
        if now >= self.expires_at {
            return Validity::Expired;
        }
        if self.last_seen_at.millis_until(now) >= IDLE_TIMEOUT_MILLIS {
            return Validity::Idle;
        }
        Validity::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_at(created: i64, last_seen: i64) -> Session {
        Session {
            digest: "d".to_owned(),
            user_id: "U".to_owned(),
            created_at: Timestamp::from_millis(created),
            expires_at: Timestamp::from_millis(created + ABSOLUTE_LIFETIME_MILLIS),
            last_seen_at: Timestamp::from_millis(last_seen),
            user_agent: None,
            ip: None,
            revoked_at: None,
        }
    }

    #[test]
    fn a_session_seen_a_moment_ago_is_active() {
        let session = session_at(0, 0);
        assert_eq!(
            session.validity(Timestamp::from_millis(1)),
            Validity::Active
        );
    }

    #[test]
    fn the_idle_window_lapses_exactly_at_seven_days() {
        let session = session_at(0, 0);
        assert_eq!(
            session.validity(Timestamp::from_millis(IDLE_TIMEOUT_MILLIS - 1)),
            Validity::Active
        );
        assert_eq!(
            session.validity(Timestamp::from_millis(IDLE_TIMEOUT_MILLIS)),
            Validity::Idle
        );
    }

    #[test]
    fn activity_does_not_move_the_thirty_day_ceiling() {
        // Seen a millisecond ago, and still expired: the absolute timeout has
        // no extension, which is the whole point of having two of them.
        let session = session_at(0, ABSOLUTE_LIFETIME_MILLIS - 1);
        assert_eq!(
            session.validity(Timestamp::from_millis(ABSOLUTE_LIFETIME_MILLIS)),
            Validity::Expired
        );
    }

    #[test]
    fn revocation_outranks_every_other_reason() {
        let mut session = session_at(0, 0);
        session.revoked_at = Some(Timestamp::from_millis(1));
        assert_eq!(
            session.validity(Timestamp::from_millis(ABSOLUTE_LIFETIME_MILLIS * 2)),
            Validity::Revoked
        );
    }
}
