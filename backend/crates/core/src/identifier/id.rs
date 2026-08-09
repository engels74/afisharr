// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The ULID primary key.

use std::fmt;

use thiserror::Error;
use ulid::Ulid;

use crate::{
    entropy,
    time::{Clock, Timestamp},
};

/// Why a string is not a ULID.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum IdError {
    /// The text was not 26 characters of Crockford base32.
    #[error("not a ULID: {0}")]
    Malformed(String),
}

/// A 26-character Crockford base32 ULID, uppercase.
///
/// ULIDs sort lexicographically by creation time, so `ORDER BY id` is
/// `ORDER BY created_at` with no second column. Placement leans on that: it
/// breaks ordering ties by ULID ascending precisely because the tie-break must
/// be stable across passes and independent of any mutable field (PRD §19.1).
///
/// ```
/// use afisharr_core::identifier::Id;
///
/// let id = Id::parse("00000000000000000000000001").expect("a valid ULID literal");
/// assert_eq!(id.as_str(), "00000000000000000000000001");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id {
    text: String,
}

impl Id {
    /// Mints a new identifier whose timestamp component comes from `clock`.
    ///
    /// The clock is injected rather than read so a test can mint identifiers
    /// with a chosen ordering.
    pub fn generate(clock: &impl Clock) -> Self {
        let millis = u64::try_from(clock.now().as_millis()).unwrap_or(0);
        Self {
            text: Ulid::from_parts(millis, rand_128()).to_string(),
        }
    }

    /// Parses text that is expected to be a ULID.
    ///
    /// # Errors
    /// Returns [`IdError::Malformed`] when `text` is not 26 characters of
    /// Crockford base32.
    pub fn parse(text: &str) -> Result<Self, IdError> {
        Ulid::from_string(text)
            .map(|ulid| Self {
                text: ulid.to_string(),
            })
            .map_err(|_| IdError::Malformed(text.to_owned()))
    }

    /// The canonical text, ready to bind to a `TEXT` primary key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The instant encoded in the identifier's timestamp component.
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        let millis = Ulid::from_string(&self.text).map_or(0, |ulid| ulid.timestamp_ms());
        Timestamp::from_millis(i64::try_from(millis).unwrap_or(i64::MAX))
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// The 80 random bits of a ULID, drawn from the OS CSPRNG.
fn rand_128() -> u128 {
    u128::from_be_bytes(entropy::bytes::<16>())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::time::FixedClock;

    #[test]
    fn generated_ids_are_26_characters() {
        let clock = FixedClock::at(Timestamp::from_millis(1_700_000_000_000));
        assert_eq!(Id::generate(&clock).as_str().len(), 26);
    }

    #[test]
    fn generated_ids_sort_by_the_clock_that_minted_them() {
        let clock = FixedClock::at(Timestamp::from_millis(1_700_000_000_000));
        let earlier = Id::generate(&clock);
        clock.advance(1);
        let later = Id::generate(&clock);
        assert!(earlier < later, "{earlier} should sort before {later}");
    }

    #[test]
    fn created_at_round_trips_through_the_timestamp_component() {
        let minted_at = Timestamp::from_millis(1_700_000_000_000);
        let clock = FixedClock::at(minted_at);
        assert_eq!(Id::generate(&clock).created_at(), minted_at);
    }

    #[test]
    fn parse_rejects_a_string_that_is_not_a_ulid() {
        assert_eq!(
            Id::parse("not-a-ulid"),
            Err(IdError::Malformed("not-a-ulid".to_owned()))
        );
    }

    #[test]
    fn parse_rejects_crockford_excluded_letters() {
        // I, L, O and U are excluded from Crockford base32 to keep the alphabet
        // unambiguous when a human transcribes it.
        assert!(Id::parse("0000000000000000000000000I").is_err());
    }
}
