// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Answering the question a listing call was actually asked.
//!
//! The client builds Plex's operator suffixes correctly and the fake filtered
//! on none of them, so every filter test was a test of a URL: the request was
//! right, the answer was the whole library, and the assertion passed because
//! the whole library contained what was asked for.
//!
//! Two things this has to get right or it is worse than nothing:
//!
//! - The conjunction and the disjunction must be observably different.
//!   `genre=comedy,drama` asks for either and `genre&=comedy&genre&=drama`
//!   asks for both. A fake that treated them alike would pass the one client
//!   bug the two spellings exist to prevent.
//! - A field the fake does not filter on is ignored rather than treated as
//!   matching nothing. An unknown argument answering an empty library is a
//!   fetch failure wearing the shape of an empty result (`I-SRC-1`).

use crate::fake::{request::Arguments, state::FakeItem, vocabulary::GENRES};

/// The fields the fake filters on — the ones its own vocabulary declares.
const FILTERED: [&str; 4] = ["genre", "year", "title", "label"];

/// How one filter argument compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operator {
    /// `=` — any of the values.
    Any,
    /// `!` — none of the values.
    None,
    /// `=` doubled — exactly one of the values, rather than a contains match.
    Exact,
    /// `!=` — not exactly any of the values.
    NotExact,
    /// `>>` — at or above.
    AtLeast,
    /// `<<` — at or below.
    AtMost,
    /// `&` — every value, rather than any.
    All,
}

impl Operator {
    /// Splits a query key into its field and its operator.
    ///
    /// The longest suffix first: `!=` before `!`, or `title!=` would read as
    /// the field `title=` compared with `!`.
    fn split(key: &str) -> (&str, Self) {
        for (suffix, operator) in [
            ("!=", Self::NotExact),
            (">>", Self::AtLeast),
            ("<<", Self::AtMost),
            ("!", Self::None),
            ("=", Self::Exact),
            ("&", Self::All),
        ] {
            if let Some(field) = key.strip_suffix(suffix) {
                return (field, operator);
            }
        }
        (key, Self::Any)
    }
}

/// One filter argument, resolved against the fields the fake knows.
#[derive(Debug, Clone)]
struct Predicate {
    field: String,
    operator: Operator,
    values: Vec<String>,
}

impl Predicate {
    /// Whether one item satisfies this predicate.
    fn matches(&self, item: &FakeItem) -> bool {
        match self.field.as_str() {
            "genre" => self.tags(&item.genres),
            "label" => self.tags(&item.labels),
            "year" => self.number(item.year),
            "title" => self.text(&item.title),
            // Unreachable: nothing outside `FILTERED` builds a predicate.
            _ => true,
        }
    }

    /// A tag comparison, over the values a choice list resolves to.
    fn tags(&self, carried: &[String]) -> bool {
        let wanted: Vec<String> = self.values.iter().map(|value| tag_title(value)).collect();
        let holds = |value: &String| carried.iter().any(|tag| tag.eq_ignore_ascii_case(value));
        match self.operator {
            Operator::All => wanted.iter().all(holds),
            Operator::None | Operator::NotExact => !wanted.iter().any(holds),
            _ => wanted.iter().any(holds),
        }
    }

    /// A numeric comparison.
    fn number(&self, carried: Option<i32>) -> bool {
        // An item with no value for the field is not a match, and is not a
        // failure either: `year>>=2000` on an item with no year is a question
        // the item cannot answer (P1).
        let Some(carried) = carried else {
            return false;
        };
        let numbers: Vec<i32> = self
            .values
            .iter()
            .filter_map(|value| value.parse().ok())
            .collect();
        match self.operator {
            Operator::AtLeast => numbers.iter().any(|value| carried >= *value),
            Operator::AtMost => numbers.iter().any(|value| carried <= *value),
            Operator::None | Operator::NotExact => !numbers.contains(&carried),
            Operator::All => numbers.iter().all(|value| carried == *value),
            _ => numbers.contains(&carried),
        }
    }

    /// A string comparison. Plex's bare `=` is a contains match, and the
    /// doubled one is equality — which is the whole reason `==` exists.
    fn text(&self, carried: &str) -> bool {
        let lowered = carried.to_lowercase();
        let contains = |value: &String| lowered.contains(&value.to_lowercase());
        let equals = |value: &String| carried.eq_ignore_ascii_case(value);
        match self.operator {
            Operator::Exact => self.values.iter().any(equals),
            Operator::NotExact => !self.values.iter().any(equals),
            Operator::None => !self.values.iter().any(contains),
            Operator::All => self.values.iter().all(contains),
            _ => self.values.iter().any(contains),
        }
    }
}

/// The title a tag value names, whether it arrived as a key or as a title.
///
/// A client resolves a genre's name to its key through the choice list and
/// sends the key; a hand-written query sends the name. Both are the same
/// question.
fn tag_title(value: &str) -> String {
    GENRES
        .iter()
        .find(|(key, _)| *key == value)
        .map_or_else(|| value.to_owned(), |(_, title)| (*title).to_owned())
}

/// Every predicate one query carries.
fn predicates(arguments: &Arguments) -> Vec<Predicate> {
    let mut predicates: Vec<Predicate> = Vec::new();
    for (key, value) in arguments.pairs() {
        // The libtype prefix a real client sends back verbatim from the field
        // list it discovered: `movie.genre`, not `genre`.
        let bare = key.rsplit('.').next().unwrap_or(key);
        let (field, operator) = Operator::split(bare);
        if !FILTERED.contains(&field) {
            continue;
        }
        let values: Vec<String> = if operator == Operator::All {
            vec![value.clone()]
        } else {
            value.split(',').map(str::to_owned).collect()
        };
        // A repeated conjunctive key is one predicate over several values, not
        // several predicates — `genre&=93&genre&=94` asks for both at once.
        match predicates
            .iter_mut()
            .find(|existing| existing.field == field && existing.operator == operator)
        {
            Some(existing) if operator == Operator::All => existing.values.extend(values),
            _ => predicates.push(Predicate {
                field: field.to_owned(),
                operator,
                values,
            }),
        }
    }
    predicates
}

/// The sort key and direction a query asked for.
fn sort_of(arguments: &Arguments) -> Option<(String, bool)> {
    let raw = arguments.first("sort")?;
    // One key: the fake sorts by the first, which is what a test asserts on.
    let first = raw.split(',').next().unwrap_or(raw);
    let (key, direction) = first.split_once(':').unwrap_or((first, ""));
    let key = key.rsplit('.').next().unwrap_or(key);
    Some((key.to_owned(), direction == "desc"))
}

/// The items one listing call asked for, filtered and ordered.
pub(crate) fn select<'a>(items: &'a [FakeItem], arguments: &Arguments) -> Vec<&'a FakeItem> {
    let predicates = predicates(arguments);
    let mut selected: Vec<&FakeItem> = items
        .iter()
        .filter(|item| predicates.iter().all(|predicate| predicate.matches(item)))
        .collect();
    if let Some((key, descending)) = sort_of(arguments) {
        // A key the fake does not sort by leaves the library's own order,
        // which is the order a verification read has to see (§15.3).
        let ordered = match key.as_str() {
            "titleSort" | "title" => {
                selected.sort_by(|left, right| {
                    sort_title(left)
                        .cmp(&sort_title(right))
                        .then_with(|| left.rating_key.cmp(&right.rating_key))
                });
                true
            }
            "year" => {
                selected.sort_by(|left, right| {
                    left.year
                        .cmp(&right.year)
                        .then_with(|| left.rating_key.cmp(&right.rating_key))
                });
                true
            }
            "addedAt" => {
                selected.sort_by(|left, right| left.rating_key.cmp(&right.rating_key));
                true
            }
            _ => false,
        };
        // Direction included: a fake that reversed the library's own order on
        // an unrecognised key would hide a no-op move behind a re-sort.
        if ordered && descending {
            selected.reverse();
        }
    }
    selected
}

/// What an item sorts under: its sort title, or its title when it has none.
///
/// The substitution a client makes for display, and the reason the *capture*
/// reads presence off the raw attribute instead (§15.6).
fn sort_title(item: &FakeItem) -> String {
    item.sort_title
        .clone()
        .unwrap_or_else(|| item.title.clone())
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{library::World, scenario::Scenario};

    fn items() -> Vec<FakeItem> {
        World::build(&Scenario::behaving(1).holding(12, 0))
            .libraries
            .swap_remove(0)
            .items
    }

    fn keys(query: &str) -> Vec<String> {
        let items = items();
        select(&items, &Arguments::parse(Some(query)))
            .into_iter()
            .map(|item| item.rating_key.clone())
            .collect()
    }

    #[test]
    fn a_query_with_no_filters_is_the_whole_library_in_its_own_order() {
        assert_eq!(keys("").len(), 12);
        assert_eq!(keys("")[0], "10001");
    }

    #[test]
    fn a_tag_filter_answers_the_items_carrying_that_tag() {
        // Sent as the key a choice list resolved to, which is how a real client
        // sends it (`plexapi/library.py:1178`).
        let comedies = keys("genre=93");
        assert_eq!(comedies.len(), 4);
        assert_eq!(keys("genre=Comedy"), comedies, "the name asks the same");
    }

    #[test]
    fn a_dotted_field_key_is_the_same_filter_as_a_bare_one() {
        // The reference client sends back the field key it discovered, dotted.
        assert_eq!(keys("movie.genre=93"), keys("genre=93"));
    }

    #[test]
    fn the_conjunction_and_the_disjunction_are_observably_different() {
        // The whole reason the two are spelled apart. Comma-joined asks for
        // either; repeated `&=` asks for both, and nothing carries both.
        // The `&` is part of the *key* and arrives quoted, which is what keeps
        // it from being read as another argument separator.
        assert_eq!(keys("genre=93,94").len(), 8);
        assert!(keys("genre%26=93&genre%26=94").is_empty());
    }

    #[test]
    fn a_negated_tag_filter_excludes_rather_than_including() {
        assert_eq!(keys("genre!=93").len(), 8);
    }

    #[test]
    fn a_numeric_range_filter_compares_rather_than_matching() {
        let items = items();
        let above = keys("year>>=1990");
        assert!(!above.is_empty());
        for key in &above {
            let year = items
                .iter()
                .find(|item| &item.rating_key == key)
                .and_then(|item| item.year)
                .expect("every item has a year");
            assert!(year >= 1990, "{key} is from {year}");
        }
        assert!(keys("year<<=1980").len() < above.len() + 12);
    }

    #[test]
    fn a_bare_string_filter_contains_and_a_doubled_one_is_exact() {
        // The doubled `=` is part of the key and arrives quoted; unquoted it
        // would be read as the separator and the operator would vanish.
        assert_eq!(keys("title=Film+1").len(), 4, "Film 1, 10, 11, 12");
        assert_eq!(keys("title%3D=Film+1").len(), 1);
        assert_eq!(keys("title%21%3D=Film+1").len(), 11);
    }

    #[test]
    fn a_field_the_fake_does_not_filter_on_is_ignored_rather_than_matching_nothing() {
        // An unknown argument answering an empty library is a fetch failure
        // wearing the shape of an empty result (`I-SRC-1`).
        assert_eq!(keys("contentRating=R").len(), 12);
        assert_eq!(keys("includeGuids=1").len(), 12);
    }

    #[test]
    fn a_sort_orders_the_answer_and_a_direction_reverses_it() {
        let ascending = keys("sort=movie.titleSort:asc");
        let descending = keys("sort=movie.titleSort:desc");
        assert_eq!(ascending.len(), 12);
        assert_eq!(
            descending,
            ascending.iter().rev().cloned().collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_sort_key_the_fake_does_not_know_leaves_the_order_alone() {
        // The library's own order is what a verification read has to see, so a
        // fake that invented an order for an unknown key would hide a no-op
        // move behind a re-sort (§15.3).
        assert_eq!(keys("sort=nonsense:desc"), keys(""));
    }
}
