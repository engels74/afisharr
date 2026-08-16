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
/// Facts a real server withholds unless the request asks, and the fake could
/// not previously produce any of them. `accessible` and `exists` need a file
/// check the request has to ask for (`plexapi/media.py:110-112`), the external
/// ids need `includeGuids` (`plexapi/library.py:1266`,
/// `plexapi/base.py:209` — a reference client sends it on every listing *and*
/// every detail fetch, which is the only evidence in reach that the answer
/// turns on it), and the optional media attributes are simply not always there.
/// A client that read an absent one as `false`, or an unasked-for one as
/// "this item has no external ids", would be stating a fact nobody gave it
/// (P1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Detail {
    /// Whether the request asked for a file check.
    pub(crate) check_files: bool,
    /// Whether the request asked for the external ids.
    ///
    /// Answered unconditionally before, so a client that never sent the
    /// argument read external ids here and none at all from a real server —
    /// and the fake was what hid the missing argument.
    pub(crate) include_guids: bool,
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
        include_guids: false,
        withhold: false,
    };
}
