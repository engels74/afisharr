// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The jailed filesystem browser.
//!
//! The containment rule lives in `afisharr_core::filesystem` and is shared with
//! the placeholder writer (`I-SEC-4`); this module is the HTTP surface over it
//! and decides nothing about paths itself.

pub(crate) mod browse;

pub use browse::{BrowseQuery, DirectoryListing, RootView, browse, roots};
