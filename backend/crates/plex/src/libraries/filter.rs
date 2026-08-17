// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The query arguments a library search is expressed in.
//!
//! Plex expresses an operator as a suffix on the field key rather than as a
//! separate parameter, so `year>>=2000` is the key `year>>` carrying the value
//! `2000`. Writing that by hand at each call site is how a `>=` that should
//! have been `>>=` reaches a server, which answers 200 and a wrong result set —
//! so the suffixes live here, once, as a closed set (PRD §13.2.4, P7).

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
