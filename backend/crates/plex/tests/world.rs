// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A world wide enough to fail against.
//!
//! The fake's world was fixed at two libraries keyed `1` and `2`, which put a
//! section-key change, a second movie library, and a music library all out of
//! reach — and left PRD §19.7's uuid-first matching with nothing to match
//! against. A scenario now declares what the server holds.
//!
//! Everything here is still drawn from one seed in a fixed order, because that
//! is the property the whole fake rests on: a wider world is more draws from
//! the same stream, never a different stream.

mod harness;

use afisharr_plex::{
    fake::{FakePlex, LibrarySpec, Scenario},
    libraries::{ItemQuery, LibraryKind, SectionKey, Window},
};
use harness::client_for;

/// A transcript of everything one run of a scenario answers.
///
/// Compared byte for byte between two runs, which is what makes "the same seed
/// replays" a checkable claim rather than an intention.
async fn transcript(scenario: Scenario) -> String {
    let fake = FakePlex::start(scenario).await;
    let client = client_for(&fake);
    let mut lines = Vec::new();
    for section in client.sections().await.expect("the fake answers") {
        lines.push(format!(
            "{}|{}|{}|{:?}",
            section.key,
            section.uuid.clone().unwrap_or_default(),
            section.title,
            section.kind
        ));
        let page = client
            .items(&section.key, &ItemQuery::new(Window::first(500)))
            .await
            .expect("the fake answers");
        for item in page.items {
            lines.push(format!(
                "  {}|{}|{:?}|{}|{:?}",
                item.rating_key,
                item.guid.unwrap_or_default(),
                item.sort_title.value(),
                item.sort_title.is_locked(),
                item.thumb.map(|thumb| thumb.as_str().to_owned()),
            ));
        }
    }
    lines.join("\n")
}

/// Three libraries, one of them music, under keys nothing else uses.
fn three_libraries() -> Scenario {
    Scenario::behaving(2024)
        .with_libraries([
            LibrarySpec::of("7", "movie", "Films").holding(8),
            LibrarySpec::of("8", "movie", "Documentaries").holding(4),
            LibrarySpec::of("9", "artist", "Music").holding(3),
        ])
        .unrecognised_artwork(3)
        .absent_sort_titles(4)
        .locked_sort_titles(5)
}

#[tokio::test]
async fn a_scenario_can_declare_three_libraries_including_one_this_build_never_manages() {
    // A music library is representable and never managed (PRD §19.7), and the
    // operator has to be able to see it exists — which needs it to exist.
    let fake = FakePlex::start(three_libraries()).await;
    let sections = client_for(&fake)
        .sections()
        .await
        .expect("the fake answers");

    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0].key, SectionKey::new("7"));
    assert_eq!(sections[1].kind, LibraryKind::Movie);
    assert_eq!(sections[2].kind, LibraryKind::Artist);
    assert_eq!(sections[2].agent.as_deref(), Some("tv.plex.agents.music"));
    assert_eq!(sections[2].scanner.as_deref(), Some("Plex Music"));
    assert_ne!(
        sections[0].uuid, sections[1].uuid,
        "two movie libraries are two libraries"
    );
}

#[tokio::test]
async fn a_section_key_can_change_mid_test_and_be_detected() {
    // The same class of failure as a changed machine identifier, one level
    // down: every stored section key now addresses something else, and `uuid`
    // is what PRD §19.7 matches on first.
    let fake = FakePlex::start(three_libraries()).await;
    let client = client_for(&fake);

    let before = client.sections().await.expect("the fake answers");
    let films = before
        .iter()
        .find(|section| section.title == "Films")
        .expect("the library is there")
        .clone();
    assert_eq!(films.key, SectionKey::new("7"));

    assert!(fake.section_key_becomes("7", "42"));

    let after = client.sections().await.expect("the fake answers");
    let rebound = after
        .iter()
        .find(|section| section.uuid == films.uuid)
        .expect("the uuid is what survives the key change");
    assert_eq!(rebound.key, SectionKey::new("42"));
    assert!(
        after.iter().all(|section| section.key != films.key),
        "nothing answers to the old key any more"
    );

    // And the old key is a key the server does not hold, which it says.
    let error = client
        .items(&films.key, &ItemQuery::new(Window::first(10)))
        .await
        .expect_err("the old key addresses nothing");
    assert_eq!(error.refused_status(), Some(404));
}

#[tokio::test]
async fn one_item_can_be_re_keyed_while_every_other_key_stays() {
    // The case that breaks a cache. A wholesale churn is detectable by a
    // caller comparing two whole windows; this one is not (`I-ID-1`).
    let fake = FakePlex::start(Scenario::behaving(1).holding(6, 0)).await;
    let client = client_for(&fake);
    let section = SectionKey::new("1");

    let before = client
        .items(&section, &ItemQuery::new(Window::first(100)))
        .await
        .expect("the fake answers");
    let moved = before.items[2].rating_key.clone();

    fake.churn_one_rating_key(moved.as_str());

    let after = client
        .items(&section, &ItemQuery::new(Window::first(100)))
        .await
        .expect("the fake answers");
    assert_eq!(after.items.len(), before.items.len());
    for (index, item) in after.items.iter().enumerate() {
        if index == 2 {
            assert_ne!(item.rating_key, moved, "this one moved");
        } else {
            assert_eq!(
                item.rating_key, before.items[index].rating_key,
                "and nothing else did"
            );
        }
        assert_eq!(
            item.guid, before.items[index].guid,
            "the guid is the identity, and it survives"
        );
    }
}

#[tokio::test]
async fn two_runs_of_a_wider_world_from_one_seed_are_byte_identical() {
    // The property every row of the fidelity contract rests on. A wider world
    // is more draws from the same stream in a fixed order, so the assertion
    // has to hold over the new shapes too or the seed means nothing.
    let first = transcript(three_libraries()).await;
    let second = transcript(three_libraries()).await;
    assert_eq!(first, second, "the same seed must replay byte for byte");

    let other = transcript(three_libraries().with_libraries([
        LibrarySpec::of("7", "movie", "Films").holding(8),
        LibrarySpec::of("8", "movie", "Documentaries").holding(4),
        LibrarySpec::of("9", "artist", "Music").holding(3),
    ]))
    .await;
    assert_eq!(first, other, "the same declaration is the same world");
}

#[tokio::test]
async fn a_different_seed_is_a_different_world_even_with_the_same_libraries() {
    let one = transcript(three_libraries()).await;
    let two = transcript(three_libraries().reseeded(2025)).await;
    assert_ne!(one, two, "or the seed means nothing");
}

#[tokio::test]
async fn a_smart_collection_says_so_rather_than_being_unreachable() {
    // The refusals a smart collection produces live in the client that reads
    // it (`plexapi/collection.py:317-318`), and nothing could reach them while
    // the fake had no way to mark one.
    let fake = FakePlex::start(
        Scenario::behaving(1).with_libraries([LibrarySpec::of("1", "movie", "Movies")
            .holding(6)
            .with_smart_collection()]),
    )
    .await;
    let collections = client_for(&fake)
        .collections(&SectionKey::new("1"))
        .await
        .expect("the fake answers");
    assert!(collections[0].smart);
}
