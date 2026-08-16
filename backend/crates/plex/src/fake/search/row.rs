// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a listing call can filter and order, whatever kind of row it is.

use crate::fake::state::{FakeCollection, FakeItem};

/// A row a listing call can filter and order.
///
/// Collections are rows on the same two endpoints items are: `type=18` on
/// `/library/sections/{key}/all` asks for them
/// (`plexapi/library.py:1666-1670`), and the `collection` libtype declares its
/// own `label` filter (`plexapi/library.py:890-899`). Filtering one kind of row
/// and handing back every row of the other is the same failure the parent
/// module exists to remove, one libtype short: the request is right, the answer
/// is the whole list, and the assertion passes because the whole list contains
/// what was asked for.
pub(crate) trait Row {
    /// Plex's key for the row, which breaks a tie in a sort.
    fn rating_key(&self) -> &str;
    fn title(&self) -> &str;
    fn sort_title(&self) -> Option<&str>;
    /// A row with no year matches no year filter, which is not the same as
    /// failing one (P1). A collection has none.
    fn year(&self) -> Option<i32> {
        None
    }
    fn genres(&self) -> &[String] {
        &[]
    }
    fn labels(&self) -> &[String];
}

impl Row for FakeItem {
    fn rating_key(&self) -> &str {
        &self.rating_key
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn sort_title(&self) -> Option<&str> {
        self.sort_title.as_deref()
    }
    fn year(&self) -> Option<i32> {
        self.year
    }
    fn genres(&self) -> &[String] {
        &self.genres
    }
    fn labels(&self) -> &[String] {
        &self.labels
    }
}

impl Row for FakeCollection {
    fn rating_key(&self) -> &str {
        &self.rating_key
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn sort_title(&self) -> Option<&str> {
        self.sort_title.as_deref()
    }
    fn labels(&self) -> &[String] {
        &self.labels
    }
}
