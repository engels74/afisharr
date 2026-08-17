// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One filter argument, and what satisfies it.
//!
//! The conjunction and the disjunction must be observably different.
//! `genre=comedy,drama` asks for either and `genre&=comedy&genre&=drama` asks
//! for both. A reader that treated them alike would pass the one client bug the
//! two spellings exist to prevent.

use crate::fake::{request::Arguments, search::row::Row, vocabulary::GENRES};

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
pub(super) struct Predicate {
    field: String,
    operator: Operator,
    values: Vec<String>,
}

impl Predicate {
    /// Whether one row satisfies this predicate.
    pub(super) fn matches(&self, row: &impl Row) -> bool {
        match self.field.as_str() {
            // Only the genre choice list has numeric keys, so only a genre
            // value is resolved through it. Resolving a label the same way
            // would answer `label=93` with everything tagged `Comedy`.
            "genre" => self.tags(row.genres(), true),
            "label" => self.tags(row.labels(), false),
            "year" => self.number(row.year()),
            "title" => self.text(row.title()),
            // Unreachable: nothing outside `FILTERED` builds a predicate.
            _ => true,
        }
    }

    /// A tag comparison, over the values a choice list resolves to.
    fn tags(&self, carried: &[String], resolve: bool) -> bool {
        let wanted: Vec<String> = self
            .values
            .iter()
            .map(|value| {
                if resolve {
                    tag_title(value)
                } else {
                    value.clone()
                }
            })
            .collect();
        let holds = |value: &String| carried.iter().any(|tag| tag.eq_ignore_ascii_case(value));
        match self.operator {
            Operator::All => wanted.iter().all(holds),
            Operator::None | Operator::NotExact => !wanted.iter().any(holds),
            _ => wanted.iter().any(holds),
        }
    }

    /// A numeric comparison.
    fn number(&self, carried: Option<i32>) -> bool {
        // A row with no value for the field is not a match, and is not a
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
pub(super) fn predicates(arguments: &Arguments) -> Vec<Predicate> {
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
