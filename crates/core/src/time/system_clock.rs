// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The clock that reads the operating system.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::time::{Clock, Timestamp};

/// Reads the host's wall clock. The only implementation that does.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        // A host clock set before 1970 is a misconfigured machine, not a case to
        // model: it yields a negative millisecond count, which every comparison
        // in the product still orders correctly.
        let millis = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(since) => i64::try_from(since.as_millis()).unwrap_or(i64::MAX),
            Err(before) => -i64::try_from(before.duration().as_millis()).unwrap_or(i64::MAX),
        };
        Timestamp::from_millis(millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_after_the_epoch_on_a_correctly_configured_host() {
        assert!(SystemClock.now() > Timestamp::EPOCH);
    }
}
