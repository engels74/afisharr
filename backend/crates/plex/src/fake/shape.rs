// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The answers the fake describes, one module per thing it answers about.
//!
//! Described once, as an [`crate::fake::element::Element`], and rendered as
//! XML or as JSON by [`crate::fake::xml`] and [`crate::fake::json`]. Nothing
//! here writes a format.
//!
//! **These shapes are claims about a server nobody in this repository
//! controls.** Each one is written against `python-plexapi` 4.18.2 — the
//! reference this phase treats as the source of truth for server behaviour —
//! and the release-lane contract test is what keeps the claim honest against a
//! real Plex.

mod collection;
mod container;
mod hub;
mod item;
mod media;
mod section;

pub(crate) use collection::collection;
pub(crate) use container::{container, library_container};
pub(crate) use hub::hub;
pub(crate) use item::item;
pub(crate) use section::section;

/// What one answer is allowed to report.
///
/// Two facts a real server withholds, and the fake could not previously
/// produce either. `accessible` and `exists` need a file check the request has
/// to ask for (`plexapi/media.py:110-112`), and the optional media attributes
/// are simply not always there. A client that read an absent one as `false`
/// would be stating a fact nobody gave it (P1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Detail {
    /// Whether the request asked for a file check.
    pub(crate) check_files: bool,
    /// Whether the scenario withholds the sometimes-reported attributes.
    pub(crate) withhold: bool,
}

impl Detail {
    /// What an ordinary request is told.
    ///
    /// Only the tests name it: every handler builds a [`Detail`] from the
    /// request it was actually given.
    #[cfg(test)]
    pub(crate) const PLAIN: Self = Self {
        check_files: false,
        withhold: false,
    };
}
