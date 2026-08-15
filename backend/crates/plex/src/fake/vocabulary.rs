// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The filter vocabulary the fake declares about itself.
//!
//! Small on purpose. The point is not to mirror a real server's field list —
//! that changes every release, and the contract test is what checks the shape.
//! The point is that the vocabulary is *discovered*: a filter carries the
//! endpoint its choices come from, a field carries the type its operators are
//! looked up by, and the operator list is the server's rather than a
//! compiled-in allowlist (PRD §13.2.4).

use serde_json::{Value, json};

use crate::fake::json as shape;

/// The library type a numeric `type=` argument names.
///
/// The vocabulary is per-libtype, and a fake that always described movies would
/// answer a show library's discovery call with a filtering type the caller can
/// never match its own libtype against — so the call would look answered and
/// the type list would be about something else.
fn libtype(plex_type: Option<&str>) -> &'static str {
    match plex_type {
        Some("2") => "show",
        Some("3") => "season",
        Some("4") => "episode",
        Some("18") => "collection",
        _ => "movie",
    }
}

/// `GET /library/sections/{key}/all?includeMeta=1`.
pub(crate) fn describe(section: &str, plex_type: Option<&str>) -> Value {
    let kind = libtype(plex_type);
    let asked = plex_type.unwrap_or("1");
    shape::container(&json!({
        "size": 0,
        "Meta": {
            "Type": [{
                "type": kind,
                "title": kind,
                "Filter": [
                    {
                        "filter": "genre",
                        "filterType": "string",
                        "title": "Genre",
                        // The server composes this, query string and all, and
                        // the type it carries is the one it was asked about.
                        "key": format!("/library/sections/{section}/genre?type={asked}"),
                    },
                    { "filter": "year", "filterType": "integer", "title": "Year" }
                ],
                "Sort": [
                    { "key": "titleSort", "title": "Title", "defaultDirection": "asc" },
                    { "key": "addedAt", "title": "Date Added", "defaultDirection": "desc" }
                ],
                "Field": [
                    { "key": "genre", "type": "tag", "title": "Genre" },
                    { "key": "year", "type": "integer", "title": "Year" },
                    { "key": "userRating", "type": "integer", "subType": "rating",
                      "title": "Rating" },
                    { "key": "audioLanguage", "type": "string", "title": "Audio Language" }
                ]
            }],
            "FieldType": [
                {
                    "type": "tag",
                    "Operator": [
                        { "key": "=", "title": "is" },
                        { "key": "!=", "title": "is not" }
                    ]
                },
                {
                    "type": "integer",
                    "Operator": [
                        { "key": "=", "title": "is" },
                        { "key": "!=", "title": "is not" },
                        { "key": ">>=", "title": "is at least" },
                        { "key": "<<=", "title": "is at most" }
                    ]
                },
                {
                    "type": "string",
                    "Operator": [
                        { "key": "=", "title": "contains" },
                        { "key": "==", "title": "is" },
                        { "key": "!=", "title": "does not contain" }
                    ]
                }
            ]
        }
    }))
}

/// `GET /library/sections/{key}/{filter}` — the choices for one filter.
pub(crate) fn choices(section: &str, filter: &str) -> Value {
    // Only the filter that declared a choice endpoint has choices. Answering a
    // list for one that did not would let a client that ignores the declaration
    // pass against the fake and fail against a real server.
    if filter != "genre" {
        return shape::container(&json!({ "size": 0 }));
    }
    let directory: Vec<Value> = [("93", "Comedy"), ("94", "Drama"), ("95", "Science Fiction")]
        .into_iter()
        .map(|(key, title)| {
            json!({
                "key": key,
                "title": title,
                "fastKey": format!("/library/sections/{section}/all?genre={key}"),
            })
        })
        .collect();
    shape::container(&json!({
        "size": directory.len(),
        "Directory": directory,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // These assertions are about the *shape* the fake emits, and stop there.
    // Whether the client can read it is proved end to end in `tests/fake.rs`,
    // against the running server and the real parsers; whether it matches a
    // real Plex is the contract test's job (`tests/contract.rs`). A unit test
    // here that re-parsed the fake's own JSON with the crate's own types would
    // agree with itself and prove neither.

    #[test]
    fn the_vocabulary_declares_a_filtering_type_with_fields_and_sorts() {
        let meta = describe("1", Some("1"))["MediaContainer"]["Meta"].clone();
        assert_eq!(meta["Type"][0]["type"], "movie");
        assert_eq!(meta["Type"][0]["Field"][2]["subType"], "rating");
        assert_eq!(meta["Type"][0]["Sort"][0]["key"], "titleSort");
    }

    #[test]
    fn a_filter_with_choices_declares_the_endpoint_they_come_from() {
        let meta = describe("7", Some("1"))["MediaContainer"]["Meta"].clone();
        assert_eq!(
            meta["Type"][0]["Filter"][0]["key"],
            "/library/sections/7/genre?type=1"
        );
        // A free-value filter declares none, and a client that assumed every
        // filter has a choice list would request the server root.
        assert!(meta["Type"][0]["Filter"][1].get("key").is_none());
    }

    #[test]
    fn the_operator_table_differs_by_field_type() {
        // The whole point of discovery: an integer takes range operators and a
        // tag does not, and neither list is compiled into the client.
        let meta = describe("1", Some("1"))["MediaContainer"]["Meta"].clone();
        let operators = |index: usize| {
            meta["FieldType"][index]["Operator"]
                .as_array()
                .expect("an operator list")
                .iter()
                .map(|operator| operator["key"].as_str().unwrap_or_default().to_owned())
                .collect::<Vec<String>>()
        };
        assert_eq!(meta["FieldType"][0]["type"], "tag");
        assert_eq!(operators(0), ["=", "!="]);
        assert_eq!(meta["FieldType"][1]["type"], "integer");
        assert!(
            operators(1).contains(&">>=".to_owned()),
            "{:?}",
            operators(1)
        );
    }

    #[test]
    fn the_vocabulary_describes_the_type_it_was_asked_about() {
        // A show library's discovery call answered with a movie filtering type
        // is a call that looks answered and describes something else.
        let meta = describe("2", Some("2"))["MediaContainer"]["Meta"].clone();
        assert_eq!(meta["Type"][0]["type"], "show");
        assert_eq!(
            meta["Type"][0]["Filter"][0]["key"],
            "/library/sections/2/genre?type=2"
        );
    }

    #[test]
    fn only_the_filter_that_declared_an_endpoint_answers_a_choice_list() {
        let genre = choices("1", "genre");
        assert_eq!(genre["MediaContainer"]["Directory"][0]["title"], "Comedy");
        assert!(
            choices("1", "year")["MediaContainer"]
                .get("Directory")
                .is_none()
        );
    }

    #[test]
    fn a_choice_carries_the_fast_key_that_lists_matching_items() {
        let genre = choices("1", "genre");
        assert_eq!(genre["MediaContainer"]["Directory"][0]["key"], "93");
        assert_eq!(
            genre["MediaContainer"]["Directory"][0]["fastKey"],
            "/library/sections/1/all?genre=93"
        );
    }
}
