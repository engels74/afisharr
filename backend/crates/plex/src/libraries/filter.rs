// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The query arguments a library search is expressed in.
//!
//! Plex expresses an operator as a suffix on the field key rather than as a
//! separate parameter, so `year>>=2000` is the key `year>>` carrying the value
//! `2000`. Writing that by hand at each call site is how a `>=` that should
//! have been `>>=` reaches a server, which answers 200 and a wrong result set —
//! so the suffixes live here, once, as a closed set (PRD §13.2.4, P7).

use crate::libraries::ItemKind;

/// How a filter argument compares.
///
/// The closed set PRD §13.2.4 names. Which of these a given field actually
/// accepts is not this type's business: the server reports its own operator
/// list per field type, and [`crate::discovery`] reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOperator {
    /// `=` — equal to, or any of, the values given.
    Equals,
    /// `!=` — not equal to any of the values given.
    NotEquals,
    /// `==` — exact string match, rather than Plex's default fuzzy compare.
    ExactEquals,
    /// `>>=` — at or above.
    AtLeast,
    /// `<<=` — at or below.
    AtMost,
    /// `&=` — every value must match, rather than any.
    All,
}

impl FilterOperator {
    /// What Plex appends to the field key for this operator.
    ///
    /// The trailing `=` is the query string's own separator and is never part
    /// of the suffix — writing it here would produce `year>>==2000`.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Equals => "",
            Self::NotEquals => "!",
            Self::ExactEquals => "=",
            Self::AtLeast => ">>",
            Self::AtMost => "<<",
            Self::All => "&",
        }
    }

    /// Whether each value becomes its own pair rather than joining a list.
    ///
    /// `&=` is conjunctive: `genre&=comedy&genre&=drama` asks for both, while
    /// `genre=comedy,drama` asks for either. Comma-joining a conjunction would
    /// quietly turn "and" into "or", which is a wrong collection nobody can see
    /// is wrong.
    const fn repeats_per_value(self) -> bool {
        matches!(self, Self::All)
    }
}

/// One filter argument: a field, an operator, and the values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterArgument {
    field: String,
    operator: FilterOperator,
    values: Vec<String>,
}

impl FilterArgument {
    /// A filter on `field` comparing with `operator` against `values`.
    #[must_use]
    pub fn new(field: impl Into<String>, operator: FilterOperator, values: Vec<String>) -> Self {
        Self {
            field: field.into(),
            operator,
            values,
        }
    }

    /// The query pairs this argument contributes.
    ///
    /// Empty when there are no values: a filter with nothing to match is not a
    /// filter that matches nothing, and emitting `genre=` would ask the server
    /// for items whose genre is the empty string (P1).
    #[must_use]
    pub fn pairs(&self) -> Vec<(String, String)> {
        if self.values.is_empty() {
            return Vec::new();
        }
        let key = format!("{}{}", self.field, self.operator.suffix());
        if self.operator.repeats_per_value() {
            return self
                .values
                .iter()
                .map(|value| (key.clone(), value.clone()))
                .collect();
        }
        vec![(key, self.values.join(","))]
    }
}

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
    #[must_use]
    pub const fn including_meta(mut self) -> Self {
        self.include_meta = true;
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

    fn value_for(pairs: &[(String, String)], key: &str) -> Option<String> {
        pairs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    }

    #[test]
    fn every_operator_carries_the_suffix_plex_expresses_it_as() {
        assert_eq!(FilterOperator::Equals.suffix(), "");
        assert_eq!(FilterOperator::NotEquals.suffix(), "!");
        assert_eq!(FilterOperator::ExactEquals.suffix(), "=");
        assert_eq!(FilterOperator::AtLeast.suffix(), ">>");
        assert_eq!(FilterOperator::AtMost.suffix(), "<<");
        assert_eq!(FilterOperator::All.suffix(), "&");
    }

    #[test]
    fn several_values_under_equals_become_one_comma_joined_pair() {
        let filter = FilterArgument::new(
            "genre",
            FilterOperator::Equals,
            vec!["comedy".to_owned(), "drama".to_owned()],
        );
        assert_eq!(
            filter.pairs(),
            vec![("genre".to_owned(), "comedy,drama".to_owned())]
        );
    }

    #[test]
    fn several_values_under_the_conjunction_become_one_pair_each() {
        // Comma-joined, this would ask for either genre. The difference is a
        // collection with the wrong contents and nothing to show it is wrong.
        let filter = FilterArgument::new(
            "genre",
            FilterOperator::All,
            vec!["comedy".to_owned(), "drama".to_owned()],
        );
        assert_eq!(
            filter.pairs(),
            vec![
                ("genre&".to_owned(), "comedy".to_owned()),
                ("genre&".to_owned(), "drama".to_owned()),
            ]
        );
    }

    #[test]
    fn a_filter_with_no_values_emits_nothing_at_all() {
        // `genre=` asks for items whose genre is the empty string, which is a
        // question nobody meant to ask and a result nobody can explain.
        let filter = FilterArgument::new("genre", FilterOperator::Equals, Vec::new());
        assert!(filter.pairs().is_empty());
    }

    #[test]
    fn a_numeric_comparison_uses_the_doubled_angle_suffix() {
        let filter = FilterArgument::new("year", FilterOperator::AtLeast, vec!["2000".to_owned()]);
        assert_eq!(
            filter.pairs(),
            vec![("year>>".to_owned(), "2000".to_owned())]
        );
    }

    #[test]
    fn an_exact_string_match_doubles_the_equals_sign() {
        let filter = FilterArgument::new(
            "title",
            FilterOperator::ExactEquals,
            vec!["Alien".to_owned()],
        );
        assert_eq!(
            filter.pairs(),
            vec![("title=".to_owned(), "Alien".to_owned())]
        );
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
    fn asking_for_the_servers_own_vocabulary_is_explicit() {
        let pairs = ItemQuery::new(Window::first(0)).including_meta().pairs();
        assert_eq!(value_for(&pairs, "includeMeta").as_deref(), Some("1"));
        assert!(value_for(&ItemQuery::new(Window::first(0)).pairs(), "includeMeta").is_none());
    }
}
