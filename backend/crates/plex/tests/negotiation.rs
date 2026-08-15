// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the fake answers, and in which of the two renderings.
//!
//! A Plex Media Server answers XML unless a request asks for JSON. That is not
//! a formatting detail: the reference client this phase is corrected against
//! sends no `Accept` header at all and parses every answer as XML, while this
//! crate's own client asks for JSON on every request. A fake that answered JSON
//! to both is a fake only one of the two readers can check, which is how the
//! fake and the client came to agree with each other and with no server on
//! earth.

mod harness;

use afisharr_plex::fake::{FakePlex, Scenario};
use harness::{ask, ask_as_a_reference_client};

#[tokio::test]
async fn a_request_that_asks_for_nothing_is_answered_in_xml() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let answer = ask_as_a_reference_client(&fake, "library/sections").await;

    assert_eq!(answer.status, 200);
    assert!(
        answer
            .body
            .starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"),
        "{}",
        answer.body
    );
    assert!(answer.body.contains("<MediaContainer "), "{}", answer.body);
    assert!(
        answer.body.contains("<Directory ") && answer.body.contains("type=\"movie\""),
        "{}",
        answer.body
    );
}

#[tokio::test]
async fn a_request_that_asks_for_json_is_answered_in_json() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let answer = ask(
        &fake,
        "library/sections",
        &[
            ("x-plex-token", harness::TOKEN),
            ("accept", "application/json"),
        ],
    )
    .await;

    let parsed: serde_json::Value = serde_json::from_str(&answer.body).expect("json parses");
    assert_eq!(parsed["MediaContainer"]["Directory"][0]["type"], "movie");
}

#[tokio::test]
async fn both_renderings_carry_the_same_facts_because_they_are_one_description() {
    // The property the whole design rests on: a field added to the description
    // appears in both, so the two cannot drift apart the way two hand-written
    // shapes did.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let xml = ask_as_a_reference_client(&fake, "hubs/sections/1/manage")
        .await
        .body;
    let json: serde_json::Value = serde_json::from_str(
        &ask(
            &fake,
            "hubs/sections/1/manage",
            &[
                ("x-plex-token", harness::TOKEN),
                ("accept", "application/json"),
            ],
        )
        .await
        .body,
    )
    .expect("json parses");

    let row = &json["MediaContainer"]["Hub"][1];
    for (name, value) in [
        ("identifier", "custom.collection.1.15001"),
        ("homeVisibility", "admin"),
        ("recommendationsVisibility", "all"),
    ] {
        assert_eq!(row[name], value, "json is missing {name}");
        assert!(
            xml.contains(&format!("{name}=\"{value}\"")),
            "xml is missing {name}: {xml}"
        );
    }
    // And a flag is one spelling in both: `1` as a JSON number, `"1"` as an
    // XML attribute, never `true` in one and `"1"` in the other.
    assert_eq!(row["deletable"], 1);
    assert!(xml.contains("deletable=\"1\""), "{xml}");
}

#[tokio::test]
async fn a_content_row_takes_the_element_name_its_kind_has_in_xml() {
    // `Video` for a film and `Directory` for a collection, both `Metadata` in
    // JSON. A client that resolves its classes from the element tag reads the
    // first pair; this crate's client reads the second.
    let fake = FakePlex::start(Scenario::behaving(1)).await;

    let items = ask_as_a_reference_client(&fake, "library/sections/1/all")
        .await
        .body;
    assert!(items.contains("<Video "), "{items}");
    assert!(!items.contains("<Metadata "), "{items}");

    let collections = ask_as_a_reference_client(&fake, "library/sections/1/collections")
        .await
        .body;
    assert!(
        collections.contains("<Directory ") && collections.contains("type=\"collection\""),
        "{collections}"
    );
}

#[tokio::test]
async fn the_vocabulary_block_is_an_object_in_json_and_an_element_in_xml() {
    // `Meta` occurs once and is addressed as an object — a client reads
    // `MediaContainer.Meta.Type`, not `MediaContainer.Meta[0].Type`.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let path = "library/sections/1/all?includeMeta=1&includeAdvanced=1&X-Plex-Container-Size=0";

    let json: serde_json::Value = serde_json::from_str(
        &ask(
            &fake,
            path,
            &[
                ("x-plex-token", harness::TOKEN),
                ("accept", "application/json"),
            ],
        )
        .await
        .body,
    )
    .expect("json parses");
    assert!(json["MediaContainer"]["Meta"].is_object());
    assert_eq!(json["MediaContainer"]["Meta"]["Type"][0]["type"], "movie");

    let xml = ask_as_a_reference_client(&fake, path).await.body;
    assert!(xml.contains("<Meta><Type "), "{xml}");
}

#[tokio::test]
async fn a_title_holding_markup_survives_both_renderings() {
    // The fake's own titles are tame, but a real library is not, and an
    // unescaped ampersand in one title makes the whole XML answer unparseable
    // — every other item in the library lost to one character.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = harness::client_for(&fake);
    client
        .edit_collection(
            &harness::movies(),
            &afisharr_plex::libraries::RatingKey::new("15001"),
            &afisharr_plex::collections::CollectionEdit {
                title: Some("Tom & \"Jerry\" <best>".to_owned()),
                ..afisharr_plex::collections::CollectionEdit::default()
            },
        )
        .await
        .expect("the fake answers");

    let xml = ask_as_a_reference_client(&fake, "library/sections/1/collections")
        .await
        .body;
    assert!(
        xml.contains("title=\"Tom &amp; &quot;Jerry&quot; &lt;best&gt;\""),
        "{xml}"
    );

    let collections = client
        .collections(&harness::movies())
        .await
        .expect("the fake answers");
    assert_eq!(collections[0].title, "Tom & \"Jerry\" <best>");
}
