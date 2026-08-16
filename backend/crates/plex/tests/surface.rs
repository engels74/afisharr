// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The questions the fake is asked, and whether it answers those questions.
//!
//! The client builds Plex's operator suffixes correctly and the fake filtered
//! on none of them, so every filter test was a test of a URL: the request was
//! right, the answer was the whole library, and the assertion passed because
//! the whole library contained what was asked for. The same held for `type`,
//! for `sort`, and for the paging window when it arrived as a header.
//!
//! The edit endpoint is here for the same reason. Plex has one over every
//! libtype, and the fake routed everything without a `label` argument to a
//! collection — so an item's sort title could not be written at all, and the
//! round trip §15.6 requires had nothing to round-trip against.

mod harness;

use afisharr_plex::{
    collections::{CollectionEdit, CollectionSort},
    fake::{FakePlex, Scenario},
    labels::LabelEdit,
    libraries::{FilterArgument, FilterOperator, ItemKind, ItemQuery, RatingKey, Window},
};
use harness::{ask_as_a_reference_client, client_for, movies};

/// Every rating key one query answers with.
async fn keys(fake: &FakePlex, query: ItemQuery) -> Vec<String> {
    client_for(fake)
        .items(&movies(), &query)
        .await
        .expect("the fake answers")
        .items
        .into_iter()
        .map(|item| item.rating_key.to_string())
        .collect()
}

fn everything() -> ItemQuery {
    ItemQuery::new(Window::first(500))
}

#[tokio::test]
async fn an_items_sort_title_round_trips_in_all_three_of_its_properties() {
    // The Task 7.7 precondition. Nothing could write either field before: the
    // edit endpoint routed every non-label argument to a collection.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);
    let key = RatingKey::new("10001");

    client
        .edit_item_sort_title(&movies(), ItemKind::Movie, &key, Some("!001 Alien"), true)
        .await
        .expect("the fake answers");
    assert_eq!(
        fake.snapshot().item_sort_title("10001"),
        Some((Some("!001 Alien".to_owned()), true))
    );

    let item = client.item(&key).await.expect("the fake answers");
    assert_eq!(item.sort_title.value(), Some("!001 Alien"));
    assert!(item.sort_title.is_locked());

    // And unlocking is a write of its own: a restore that left the field
    // locked would otherwise look correct (`I-REV-3`).
    client
        .edit_item_sort_title(&movies(), ItemKind::Movie, &key, Some("Alien"), false)
        .await
        .expect("the fake answers");
    let item = client.item(&key).await.expect("the fake answers");
    assert_eq!(item.sort_title.value(), Some("Alien"));
    assert!(!item.sort_title.is_locked());

    // And presence is its own property: clearing the field leaves the item
    // with no sort title rather than with an empty one, which is the state
    // most items start in and the one a teardown has to reach.
    client
        .edit_item_sort_title(&movies(), ItemKind::Movie, &key, None, true)
        .await
        .expect("the fake answers");
    let item = client.item(&key).await.expect("the fake answers");
    assert!(!item.sort_title.is_present());
    assert_eq!(item.sort_title.value(), None);
    assert!(
        item.sort_title.is_locked(),
        "absent and locked at once is a real state, and the one a restore gets wrong"
    );
}

#[tokio::test]
async fn a_two_label_removal_removes_two_labels() {
    // One comma-joined argument, which is what a real client sends. Read as a
    // repeated key, this removed one label and reported success for both.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);
    let key = RatingKey::new("10001");

    client
        .edit_labels(
            &movies(),
            ItemKind::Movie,
            &key,
            &LabelEdit {
                add: vec!["old".to_owned(), "older".to_owned(), "kept".to_owned()],
                remove: Vec::new(),
            },
        )
        .await
        .expect("the fake answers");
    client
        .edit_labels(
            &movies(),
            ItemKind::Movie,
            &key,
            &LabelEdit {
                add: Vec::new(),
                remove: vec!["old".to_owned(), "older".to_owned()],
            },
        )
        .await
        .expect("the fake answers");

    assert_eq!(
        fake.snapshot().labels("10001"),
        Some(vec!["kept".to_owned()])
    );
    assert_eq!(
        fake.snapshot().labels_locked("10001"),
        Some(false),
        "this build unlocks the field it writes, and the fake now honours it"
    );
}

#[tokio::test]
async fn an_edit_naming_an_id_the_server_does_not_hold_reports_that_it_wrote_nothing() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let written = client_for(&fake)
        .edit_collection(
            &movies(),
            &RatingKey::new("99999"),
            &CollectionEdit {
                title: Some("Nowhere".to_owned()),
                ..CollectionEdit::default()
            },
        )
        .await
        .expect("the call answers");
    assert_eq!(written, 0, "nothing was written and the answer says so");
}

#[tokio::test]
async fn a_new_collection_is_in_release_order_until_an_edit_switches_it() {
    // Custom order is a thing Afisharr must switch on, and a fake that started
    // there tested nothing (`plexapi/collection.py:73`).
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);
    let server = afisharr_plex::server::MachineIdentifier::new(fake.machine_identifier());

    let created = client
        .create_collection(
            &movies(),
            ItemKind::Movie,
            "Made By A Test",
            &server,
            &[RatingKey::new("10001")],
        )
        .await
        .expect("the fake answers");
    assert_eq!(created.sort, Some(CollectionSort::Release));

    client
        .edit_collection(
            &movies(),
            &created.rating_key,
            &CollectionEdit {
                sort: Some(CollectionSort::Custom),
                summary: Some("Ordered by hand".to_owned()),
                ..CollectionEdit::default()
            },
        )
        .await
        .expect("the fake answers");

    let listed = client
        .collections(&movies())
        .await
        .expect("the fake answers")
        .into_iter()
        .find(|candidate| candidate.rating_key == created.rating_key)
        .expect("the collection is in the library");
    assert_eq!(listed.sort, Some(CollectionSort::Custom));
    assert_eq!(
        fake.snapshot()
            .collection_presentation(created.rating_key.as_str())
            .map(|(_, summary)| summary),
        Some(Some("Ordered by hand".to_owned()))
    );
}

#[tokio::test]
async fn both_collection_item_path_families_answer_identically() {
    // Which family a real server serves is settled by the contract test, not
    // here. Serving both is what stops the fake asserting an answer either way.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let collections = ask_as_a_reference_client(&fake, "library/collections/15001/children")
        .await
        .body;
    let metadata = ask_as_a_reference_client(&fake, "library/metadata/15001/children")
        .await
        .body;
    assert_eq!(collections, metadata);
    assert!(collections.contains("<Video "), "{collections}");
}

#[tokio::test]
async fn a_library_answer_names_the_section_every_row_came_from() {
    // Without it a collection does not know its own section, and asking for
    // its ordering-space row requests `/hubs/sections/None/manage`.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    for path in [
        "library/sections/1/all",
        "library/sections/1/collections",
        "library/collections/15001/children",
    ] {
        let body = ask_as_a_reference_client(&fake, path).await.body;
        assert!(
            body.contains("librarySectionID=\"1\"")
                && body.contains("librarySectionUUID=\"uuid-section-1\""),
            "{path}: {body}"
        );
    }
}

#[tokio::test]
async fn asking_for_collections_answers_collections_rather_than_films() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let page = client_for(&fake)
        .items(&movies(), &everything().of_type(ItemKind::Collection))
        .await
        .expect("the fake answers");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].kind, Some(ItemKind::Collection));
    assert_eq!(page.items[0].rating_key, RatingKey::new("15001"));
}

#[tokio::test]
async fn every_filter_operator_the_client_can_build_narrows_the_answer() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let filtered = |field: &'static str, operator, values: Vec<String>| {
        everything().filtered_by(FilterArgument::new(field, operator, values))
    };

    let comedies = keys(
        &fake,
        filtered("genre", FilterOperator::Equals, vec!["93".to_owned()]),
    )
    .await;
    assert_eq!(comedies.len(), 4);

    let not_comedies = keys(
        &fake,
        filtered("genre", FilterOperator::NotEquals, vec!["93".to_owned()]),
    )
    .await;
    assert_eq!(not_comedies.len(), 8);

    let modern = keys(
        &fake,
        filtered("year", FilterOperator::AtLeast, vec!["1990".to_owned()]),
    )
    .await;
    let old = keys(
        &fake,
        filtered("year", FilterOperator::AtMost, vec!["1989".to_owned()]),
    )
    .await;
    assert_eq!(modern.len() + old.len(), 12);
    assert!(!modern.is_empty() && !old.is_empty());

    let exact = keys(
        &fake,
        filtered(
            "title",
            FilterOperator::ExactEquals,
            vec!["Film 1".to_owned()],
        ),
    )
    .await;
    assert_eq!(exact.len(), 1, "an exact match is not a contains match");
}

#[tokio::test]
async fn the_conjunction_asks_for_both_where_the_disjunction_asks_for_either() {
    // The whole reason the two spellings exist. A fake that treated them alike
    // would pass the one client bug they are there to prevent: a collection
    // with the wrong contents and nothing to show it is wrong.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let either = keys(
        &fake,
        everything().filtered_by(FilterArgument::new(
            "genre",
            FilterOperator::Equals,
            vec!["93".to_owned(), "94".to_owned()],
        )),
    )
    .await;
    let both = keys(
        &fake,
        everything().filtered_by(FilterArgument::new(
            "genre",
            FilterOperator::All,
            vec!["93".to_owned(), "94".to_owned()],
        )),
    )
    .await;
    assert_eq!(either.len(), 8);
    assert!(both.is_empty(), "nothing carries both genres");
}

#[tokio::test]
async fn a_sort_the_vocabulary_declares_is_applied() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let ascending = keys(&fake, everything().sorted_by("titleSort:asc")).await;
    let descending = keys(&fake, everything().sorted_by("titleSort:desc")).await;
    assert_eq!(ascending.len(), 12);
    assert_eq!(
        descending,
        ascending.iter().rev().cloned().collect::<Vec<String>>()
    );
}

#[tokio::test]
async fn a_window_paged_by_headers_answers_what_one_paged_by_arguments_does() {
    // The reference client pages by header on every loop and by argument
    // elsewhere. A fake that read one answered the whole library to half its
    // callers, and a paging test against it proved nothing.
    let fake = FakePlex::start(Scenario::behaving(1).holding(30, 0)).await;
    let by_argument = ask_as_a_reference_client(
        &fake,
        "library/sections/1/all?X-Plex-Container-Start=10&X-Plex-Container-Size=5",
    )
    .await
    .body;
    let by_header = harness::ask(
        &fake,
        "library/sections/1/all",
        &[
            ("x-plex-token", harness::TOKEN),
            ("x-plex-container-start", "10"),
            ("x-plex-container-size", "5"),
        ],
    )
    .await
    .body;
    assert_eq!(by_argument, by_header);
    assert!(by_argument.contains("totalSize=\"30\""), "{by_argument}");
    assert!(by_argument.contains("offset=\"10\""), "{by_argument}");
}

#[tokio::test]
async fn the_vocabulary_is_answered_on_the_collections_endpoint_too() {
    // A client loads the `collection` libtype's filters from there, and one
    // that got no `Meta` could not filter collections at all
    // (`plexapi/library.py:890-899`).
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let body = ask_as_a_reference_client(
        &fake,
        "library/sections/1/collections?includeMeta=1&includeAdvanced=1&X-Plex-Container-Size=0",
    )
    .await
    .body;
    assert!(body.contains("<Meta><Type "), "{body}");
    assert!(body.contains("type=\"collection\""), "{body}");
    assert!(body.contains("key=\"collection.label\""), "{body}");
}

#[tokio::test]
async fn a_section_declares_every_libtype_it_filters_rather_than_the_one_asked_about() {
    // A client picks its own libtype out of that list, and a list of one only
    // ever answers the caller that guessed right.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let vocabulary = client_for(&fake)
        .vocabulary(
            &afisharr_plex::libraries::SectionKey::new("2"),
            ItemKind::Show,
        )
        .await
        .expect("the fake answers");
    let declared: Vec<&str> = vocabulary
        .types
        .iter()
        .map(|kind| kind.raw_type.as_str())
        .collect();
    assert_eq!(declared, ["show", "season", "episode"]);
}

#[tokio::test]
async fn a_filters_choices_answer_only_at_the_endpoint_the_vocabulary_declared() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);
    let vocabulary = client
        .vocabulary(&movies(), ItemKind::Movie)
        .await
        .expect("the fake answers");
    let genre = vocabulary.types[0]
        .filters
        .iter()
        .find(|filter| filter.filter == "genre")
        .expect("the fake declares a genre filter")
        .clone();
    assert_eq!(
        genre.key.as_deref(),
        Some("/library/sections/1/genre?type=1")
    );

    let choices = client
        .filter_choices(&genre)
        .await
        .expect("the declared endpoint answers");
    assert_eq!(choices.len(), 3);

    // And a filter that declared none has no endpoint to ask, so a request to
    // one is a `404` rather than an invented list.
    assert_eq!(
        ask_as_a_reference_client(&fake, "library/sections/1/year")
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn a_filters_choices_answer_only_for_the_libtype_that_declared_the_filter() {
    // The server composes the libtype into the key it hands out, so the two
    // travel together. Answered without reading it, the fake gave a genre list
    // to `type=18` — and the `collection` libtype declares `label` and nothing
    // else, so a client that carried the wrong libtype through passed here and
    // would have got nothing from a real server.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    // Read off the endpoint the `collection` libtype's vocabulary comes from,
    // which is the collections call rather than `/all`
    // (`plexapi/library.py:890-899`).
    let declared = ask_as_a_reference_client(
        &fake,
        "library/sections/1/collections?includeMeta=1&includeAdvanced=1&X-Plex-Container-Size=0",
    )
    .await
    .body;
    assert!(
        declared.contains(r#"key="/library/sections/1/label?type=18""#),
        "{declared}"
    );
    assert_eq!(
        ask_as_a_reference_client(&fake, "library/sections/1/label?type=18")
            .await
            .status,
        200,
        "the endpoint the collection libtype was sent to answers"
    );

    assert_eq!(
        ask_as_a_reference_client(&fake, "library/sections/1/genre?type=18")
            .await
            .status,
        404,
        "the collection libtype declares no genre filter"
    );
    assert_eq!(
        ask_as_a_reference_client(&fake, "library/sections/1/label?type=1")
            .await
            .status,
        404,
        "and the movie libtype declares no label filter"
    );
}
