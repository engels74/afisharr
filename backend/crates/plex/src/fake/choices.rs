// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The enumerated values one filter offers.
//!
//! Answered only at the endpoint the vocabulary declared, and only for the
//! filters that declared one. A fake that answered a list for every path would
//! let a client that ignores the declaration pass here and fail against a real
//! server, which is the whole failure mode discovery exists to avoid.

use crate::fake::{element::Element, state::FakeLibrary, vocabulary::GENRES};

/// The choices of one filter, or `None` when the filter declared no endpoint.
///
/// `None` and an empty list are different answers: the first is "this filter
/// takes a typed value" and the second is "this filter has no values in this
/// library" (P1).
pub(crate) fn choices(library: &FakeLibrary, filter: &str) -> Option<Vec<Element>> {
    match filter {
        "genre" => Some(
            GENRES
                .iter()
                .map(|(key, title)| choice(library, "genre", key, title))
                .collect(),
        ),
        "label" => Some(
            labels(library)
                .iter()
                .map(|label| choice(library, "label", label, label))
                .collect(),
        ),
        _ => None,
    }
}

/// Every label anything in this library carries, in a stable order.
///
/// Read off the world rather than written down: a choice list that named a
/// label no item has would be a vocabulary describing a different library.
fn labels(library: &FakeLibrary) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for label in library.items.iter().flat_map(|item| item.labels.iter()) {
        if !seen.iter().any(|known| known == label) {
            seen.push(label.clone());
        }
    }
    seen
}

/// One choice, with the fast key that lists the items matching it.
fn choice(library: &FakeLibrary, filter: &str, key: &str, title: &str) -> Element {
    Element::named("Directory")
        .text("key", key)
        .text("title", title)
        .text(
            "fastKey",
            format!("/library/sections/{}/all?{filter}={key}", library.key),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{json, library::World, scenario::Scenario};

    fn library() -> FakeLibrary {
        World::build(&Scenario::behaving(1))
            .libraries
            .swap_remove(0)
    }

    #[test]
    fn a_declared_filter_answers_the_values_the_library_holds() {
        let library = library();
        let genres = choices(&library, "genre").expect("genre declares an endpoint");
        let rendered = json::document(&genres[0]);
        assert_eq!(rendered["Directory"]["key"], "93");
        assert_eq!(rendered["Directory"]["title"], "Comedy");
        assert_eq!(
            rendered["Directory"]["fastKey"],
            "/library/sections/1/all?genre=93"
        );
    }

    #[test]
    fn a_filter_that_declared_no_endpoint_has_no_list_rather_than_an_empty_one() {
        assert!(choices(&library(), "year").is_none());
        assert!(choices(&library(), "nonsense").is_none());
    }

    #[test]
    fn the_label_choices_are_the_labels_the_library_actually_carries() {
        let mut library = library();
        assert!(
            choices(&library, "label")
                .expect("label declares an endpoint")
                .is_empty(),
            "nothing is labelled yet"
        );
        library.items[0].labels.push("afisharr".to_owned());
        library.items[1].labels.push("afisharr".to_owned());
        let listed = choices(&library, "label").expect("label declares an endpoint");
        assert_eq!(listed.len(), 1, "one label, however many items carry it");
    }
}
