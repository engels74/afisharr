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

mod predicate;
mod row;

use crate::fake::request::Arguments;
use crate::fake::search::predicate::predicates;
pub(crate) use crate::fake::search::row::Row;

/// The sort key and direction a query asked for.
fn sort_of(arguments: &Arguments) -> Option<(String, bool)> {
    let raw = arguments.first("sort")?;
    // One key: the fake sorts by the first, which is what a test asserts on.
    let first = raw.split(',').next().unwrap_or(raw);
    let (key, direction) = first.split_once(':').unwrap_or((first, ""));
    let key = key.rsplit('.').next().unwrap_or(key);
    Some((key.to_owned(), direction == "desc"))
}

/// The rows one listing call asked for, filtered and ordered.
pub(crate) fn select<'a, T: Row>(rows: &'a [T], arguments: &Arguments) -> Vec<&'a T> {
    let predicates = predicates(arguments);
    let mut selected: Vec<&T> = rows
        .iter()
        .filter(|row| predicates.iter().all(|predicate| predicate.matches(*row)))
        .collect();
    if let Some((key, descending)) = sort_of(arguments) {
        // A key the fake does not sort by leaves the library's own order,
        // which is the order a verification read has to see (§15.3).
        let ordered = match key.as_str() {
            "titleSort" | "title" => {
                selected.sort_by(|left, right| {
                    sort_title(*left)
                        .cmp(&sort_title(*right))
                        .then_with(|| left.rating_key().cmp(right.rating_key()))
                });
                true
            }
            "year" => {
                selected.sort_by(|left, right| {
                    left.year()
                        .cmp(&right.year())
                        .then_with(|| left.rating_key().cmp(right.rating_key()))
                });
                true
            }
            "addedAt" => {
                selected.sort_by(|left, right| left.rating_key().cmp(right.rating_key()));
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

/// What a row sorts under: its sort title, or its title when it has none.
///
/// The substitution a client makes for display, and the reason the *capture*
/// reads presence off the raw attribute instead (§15.6).
fn sort_title(row: &impl Row) -> String {
    row.sort_title()
        .unwrap_or_else(|| row.title())
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{library::World, scenario::Scenario, state::FakeItem};

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
    fn a_label_value_is_not_resolved_through_the_genre_choice_list() {
        // `label=93` asks for the label spelled `93`. Resolved through the
        // genre choices it asked for everything tagged `Comedy` instead, which
        // is a different question answered confidently.
        let mut items = items();
        items[0].labels.push("93".to_owned());
        let wanted = items[0].rating_key.clone();
        let selected: Vec<String> = select(&items, &Arguments::parse(Some("label=93")))
            .into_iter()
            .map(|item| item.rating_key.clone())
            .collect();
        assert_eq!(selected, [wanted]);
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
    fn a_collection_answers_the_label_filter_its_own_libtype_declares() {
        // The `collection` libtype declares `label` and nothing else, and the
        // whole list came back whatever was asked — so an assertion that the
        // wanted collection is in the answer passed however wrong the filter.
        let mut collections = World::build(&Scenario::behaving(1))
            .libraries
            .swap_remove(0)
            .collections;
        // A second one, so the filter has something to exclude: a predicate
        // checked against a list of one passes whatever it does.
        let mut other = collections[0].clone();
        other.rating_key = "15009".to_owned();
        other.title = "Another Collection".to_owned();
        collections.push(other);
        collections[0].labels.push("afisharr".to_owned());
        let wanted = collections[0].rating_key.clone();
        let selected: Vec<String> = select(&collections, &Arguments::parse(Some("label=afisharr")))
            .into_iter()
            .map(|collection| collection.rating_key.clone())
            .collect();
        assert_eq!(selected, [wanted]);
    }

    #[test]
    fn a_collection_carries_no_year_and_answers_no_year_filter() {
        // Not a match, and not a failure either: a collection cannot answer the
        // question (P1). The list it is not in is the honest answer.
        let collections = World::build(&Scenario::behaving(1))
            .libraries
            .swap_remove(0)
            .collections;
        assert!(select(&collections, &Arguments::parse(Some("year>>=1990"))).is_empty());
        assert_eq!(
            select(&collections, &Arguments::parse(Some("sort=titleSort:desc"))).len(),
            collections.len(),
            "a sort excludes nothing"
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
