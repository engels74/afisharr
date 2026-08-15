// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Labels: the runtime marker Afisharr puts on the items it manages.
//!
//! Plex's tag edit is a whole-field write, so the two operations here are
//! expressed as Plex expresses them — add with `label[n].tag.tag`, remove with
//! `label[].tag.tag-` — rather than as a read-modify-write this crate composes.
//! A read-modify-write over a tag list is a lost update every time two passes
//! overlap, and it would silently drop a label the operator added in Plex.

mod edit;

pub use edit::LabelEdit;
