// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! An item's sort title, in the three properties §15.6 requires.

/// An item's sort title: its value, its presence, and its lock state.
///
/// All three are independent and all three round-trip. Presence is read from
/// the raw attribute rather than from a parsed value because Plex clients
/// substitute the title for a missing sort title, which makes "absent" and
/// "equal to the title" indistinguishable afterwards — and a teardown that
/// restored the substituted value would write a sort title the operator never
/// had.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SortTitle {
    value: Option<String>,
    locked: bool,
}

impl SortTitle {
    /// A sort title with `value` present and the given lock state.
    #[must_use]
    pub fn present(value: impl Into<String>, locked: bool) -> Self {
        Self {
            value: Some(value.into()),
            locked,
        }
    }

    /// A sort title Plex did not report, with the given lock state.
    #[must_use]
    pub const fn absent(locked: bool) -> Self {
        Self {
            value: None,
            locked,
        }
    }

    /// The raw value, or `None` when the attribute was absent.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    /// Whether the attribute was present at all.
    #[must_use]
    pub const fn is_present(&self) -> bool {
        self.value.is_some()
    }

    /// Whether Plex's own metadata lock is set on the field.
    ///
    /// A restore that leaves the field locked has permanently disabled the
    /// server's metadata refresh for that item, silently (`I-REV-3`).
    #[must_use]
    pub const fn is_locked(&self) -> bool {
        self.locked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_and_empty_are_two_different_facts() {
        // An empty sort title is a value somebody wrote. An absent one is a
        // field nobody has ever set, and a restore writing "" where there was
        // nothing has changed the library (P3).
        assert!(!SortTitle::absent(false).is_present());
        assert!(SortTitle::present("", false).is_present());
        assert_eq!(SortTitle::present("", false).value(), Some(""));
    }

    #[test]
    fn the_lock_is_independent_of_the_value() {
        // Absent and locked at once is a real state, and it is the one a
        // restore gets wrong.
        let locked = SortTitle::absent(true);
        assert!(!locked.is_present());
        assert!(locked.is_locked());
        assert!(!SortTitle::present("!001 Alien", false).is_locked());
    }

    #[test]
    fn the_default_is_absent_and_unlocked() {
        assert_eq!(SortTitle::default(), SortTitle::absent(false));
    }
}
