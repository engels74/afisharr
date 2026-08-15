// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One library search: what it asks for, and how much of it at a time.

use crate::libraries::{FilterArgument, ItemKind};

/// A bounded slice of a result set.
///
/// `I-PERF-1` forbids holding a library in memory, so every listing call takes
/// one of these and the caller advances it. There is no unwindowed variant on
/// purpose: an omitted window is a full 200,000-item fetch, and the way to stop
/// writing one is to make it unspellable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// The offset of the first item wanted.
    pub start: u32,
    /// How many items to return.
    pub size: u32,
}

impl Window {
    /// The first page of `size` items.
    #[must_use]
    pub const fn first(size: u32) -> Self {
        Self { start: 0, size }
    }

    /// The window that follows this one.
    #[must_use]
    pub const fn next(self) -> Self {
        Self {
            start: self.start.saturating_add(self.size),
            size: self.size,
        }
    }
}

/// A library search, as query arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemQuery {
    libtype: Option<ItemKind>,
    filters: Vec<FilterArgument>,
    sort: Option<String>,
    window: Window,
    include_meta: bool,
    check_files: bool,
}

impl ItemQuery {
    /// A query over one window of a library.
    #[must_use]
    pub const fn new(window: Window) -> Self {
        Self {
            libtype: None,
            filters: Vec::new(),
            sort: None,
            window,
            include_meta: false,
            check_files: false,
        }
    }

    /// Restricts the query to one item type.
    #[must_use]
    pub fn of_type(mut self, libtype: ItemKind) -> Self {
        self.libtype = Some(libtype);
        self
    }

    /// Adds a filter argument.
    #[must_use]
    pub fn filtered_by(mut self, filter: FilterArgument) -> Self {
        self.filters.push(filter);
        self
    }

    /// Sorts by one of the server's own sort keys.
    #[must_use]
    pub fn sorted_by(mut self, sort: impl Into<String>) -> Self {
        self.sort = Some(sort.into());
        self
    }

    /// Asks the server to describe its own filter vocabulary alongside the
    /// result, which is what [`crate::discovery`] reads.
    ///
    /// Two arguments, not one. `includeMeta=1` asks for the block at all and
    /// `includeAdvanced=1` is what puts the field list and the operator table
    /// in it — a real server answers a short `Meta` without the second, and a
    /// client that sent only the first would build its custom filters out of a
    /// field list it never received.
    #[must_use]
    pub const fn including_meta(mut self) -> Self {
        self.include_meta = true;
        self
    }

    /// Asks the server to go and look at the files behind the result.
    ///
    /// `Part.accessible` and `Part.exists` require it: without it a real server
    /// omits both, and omitted is *unknown* rather than false. The argument is
    /// spelled out here so the difference between "Plex says the file is gone"
    /// and "nobody asked Plex to look" is a decision at the call site (P1).
    #[must_use]
    pub const fn checking_files(mut self) -> Self {
        self.check_files = true;
        self
    }

    /// The window this query covers.
    #[must_use]
    pub const fn window(&self) -> Window {
        self.window
    }

    /// Every query pair, in a stable order.
    ///
    /// Stable because the response cache keys on the URL (PRD §21.2.5): two
    /// runs of the same query that emitted their filters in a different order
    /// would be two cache entries for one question.
    #[must_use]
    pub fn pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::with_capacity(self.filters.len() + 5);
        if let Some(libtype) = self.libtype {
            pairs.push(("type".to_owned(), libtype.as_plex_type().to_string()));
        }
        for filter in &self.filters {
            pairs.extend(filter.pairs());
        }
        if let Some(sort) = &self.sort {
            pairs.push(("sort".to_owned(), sort.clone()));
        }
        if self.include_meta {
            pairs.push(("includeMeta".to_owned(), "1".to_owned()));
            pairs.push(("includeAdvanced".to_owned(), "1".to_owned()));
        }
        if self.check_files {
            pairs.push(("checkFiles".to_owned(), "1".to_owned()));
        }
        pairs.push((
            "X-Plex-Container-Start".to_owned(),
            self.window.start.to_string(),
        ));
        pairs.push((
            "X-Plex-Container-Size".to_owned(),
            self.window.size.to_string(),
        ));
        pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libraries::{FilterArgument, FilterOperator};

    fn value_for(pairs: &[(String, String)], key: &str) -> Option<String> {
        pairs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    }

    #[test]
    fn a_query_carries_its_type_sort_and_window() {
        let pairs = ItemQuery::new(Window::first(200))
            .of_type(ItemKind::Movie)
            .sorted_by("addedAt:desc")
            .filtered_by(FilterArgument::new(
                "year",
                FilterOperator::AtMost,
                vec!["1999".to_owned()],
            ))
            .pairs();
        assert_eq!(value_for(&pairs, "type").as_deref(), Some("1"));
        assert_eq!(value_for(&pairs, "sort").as_deref(), Some("addedAt:desc"));
        assert_eq!(value_for(&pairs, "year<<").as_deref(), Some("1999"));
        assert_eq!(
            value_for(&pairs, "X-Plex-Container-Start").as_deref(),
            Some("0")
        );
        assert_eq!(
            value_for(&pairs, "X-Plex-Container-Size").as_deref(),
            Some("200")
        );
    }

    #[test]
    fn the_pair_order_is_stable_across_two_identical_queries() {
        // The response cache keys on the URL. Two orders would be two entries
        // for one question, and a hit rate that quietly halves.
        let build = || {
            ItemQuery::new(Window::first(50))
                .of_type(ItemKind::Show)
                .filtered_by(FilterArgument::new(
                    "genre",
                    FilterOperator::Equals,
                    vec!["comedy".to_owned()],
                ))
                .filtered_by(FilterArgument::new(
                    "year",
                    FilterOperator::AtLeast,
                    vec!["2010".to_owned()],
                ))
                .pairs()
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn a_window_advances_by_its_own_size() {
        let first = Window::first(200);
        assert_eq!(
            first.next(),
            Window {
                start: 200,
                size: 200
            }
        );
        assert_eq!(first.next().next().start, 400);
    }

    #[test]
    fn asking_for_the_servers_own_vocabulary_asks_for_the_advanced_half_too() {
        // A real server answers a short `Meta` without `includeAdvanced=1` —
        // no field list, no operator table — so a client that sent only
        // `includeMeta=1` would discover half a vocabulary and not know it.
        let pairs = ItemQuery::new(Window::first(0)).including_meta().pairs();
        assert_eq!(value_for(&pairs, "includeMeta").as_deref(), Some("1"));
        assert_eq!(value_for(&pairs, "includeAdvanced").as_deref(), Some("1"));
        assert!(value_for(&ItemQuery::new(Window::first(0)).pairs(), "includeMeta").is_none());
    }

    #[test]
    fn asking_the_server_to_look_at_the_files_is_explicit() {
        // Without it `accessible` and `exists` are absent, and absent is
        // unknown rather than false.
        let pairs = ItemQuery::new(Window::first(50)).checking_files().pairs();
        assert_eq!(value_for(&pairs, "checkFiles").as_deref(), Some("1"));
        assert!(value_for(&ItemQuery::new(Window::first(50)).pairs(), "checkFiles").is_none());
    }
}
