// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Every call in Task 2.1's list, against a hand-rolled fixture response.
//!
//! Two assertions per call, and both are needed. The request assertion says
//! this client asks a real Plex the question it means to ask — the method, the
//! path, and the argument shapes PRD §13.2.4 names. The response assertion says
//! it reads what a real Plex answers. A test that only did the second would
//! pass against a client that sent its filters to the wrong endpoint.

mod fixtures;

use afisharr_plex::{
    artwork::ArtworkUpload,
    collections::{CollectionEdit, CollectionMode, MoveTarget, library_uri},
    discovery::DiscoveredFilter,
    hubs::{HubIdentifier, HubMove, HubVisibility},
    labels::LabelEdit,
    libraries::{
        FilterArgument, FilterOperator, ItemKind, ItemQuery, RatingKey, ScanState, SectionKey,
        Window,
    },
    server::MachineIdentifier,
};
use fixtures::FixtureServer;

fn section() -> SectionKey {
    SectionKey::new("1")
}

fn server() -> MachineIdentifier {
    MachineIdentifier::new("machine-abc")
}

#[tokio::test]
async fn the_identity_call_reads_the_machine_identifier_and_the_version() {
    let fixture = FixtureServer::answering(
        r#"{"MediaContainer":{"size":0,"claimed":true,
            "machineIdentifier":"machine-abc","version":"1.41.0.1234"}}"#,
    )
    .await;

    let identity = fixture
        .client()
        .identity()
        .await
        .expect("the fixture answers");

    let request = fixture.only_request();
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/identity");
    assert_eq!(request.token.as_deref(), Some("test-plex-token"));
    assert_eq!(request.accept.as_deref(), Some("application/json"));
    assert_eq!(
        request.client_identifier.as_deref(),
        Some("01JTESTCLIENT"),
        "plex.tv binds tokens to it and the server logs it (PRD §19.5)"
    );
    assert_eq!(
        identity.machine_identifier,
        MachineIdentifier::new("machine-abc")
    );
    assert_eq!(identity.version, "1.41.0.1234");
}

#[tokio::test]
async fn the_credential_call_asks_the_server_root_and_presents_the_token() {
    // The whole point of the call is the header. A request to the root without
    // the token is one a claimed server answers `401` to for a reason that has
    // nothing to do with the stored credential, which is the opposite of what
    // this call is asked to find out.
    let fixture = FixtureServer::answering(
        r#"{"MediaContainer":{"size":0,"machineIdentifier":"machine-abc",
            "version":"1.41.0.1234","friendlyName":"Living Room"}}"#,
    )
    .await;

    fixture
        .client()
        .verify_credential()
        .await
        .expect("the fixture answers");

    let request = fixture.only_request();
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/");
    assert_eq!(request.query, "");
    assert_eq!(request.token.as_deref(), Some("test-plex-token"));
}

#[tokio::test]
async fn a_refused_credential_call_carries_the_status_the_server_refused_with() {
    // 401 and "the server did not answer" send an operator in opposite
    // directions, and the status is the only thing that tells them apart.
    let fixture = FixtureServer::answering_with(401, r#"{"error":"unauthorized"}"#).await;

    let error = fixture
        .client()
        .verify_credential()
        .await
        .expect_err("a refused token is not a working connection");

    assert_eq!(error.refused_status(), Some(401));
    assert!(error.server_answered());
}

#[tokio::test]
async fn the_section_list_reads_every_library_including_the_unmanageable_ones() {
    let fixture = FixtureServer::answering(
        r#"{"MediaContainer":{"size":2,"Directory":[
            {"key":"1","uuid":"u-1","type":"movie","title":"Movies","agent":"tv.plex.agents.movie",
             "language":"en-US","scannedAt":1758000000},
            {"key":"3","uuid":"u-3","type":"artist","title":"Music"}]}}"#,
    )
    .await;

    let sections = fixture
        .client()
        .sections()
        .await
        .expect("the fixture answers");

    assert_eq!(fixture.only_request().path, "/library/sections");
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].key, section());
    assert_eq!(sections[0].uuid.as_deref(), Some("u-1"));
    // Refusing to represent a music library is the library cache's rule, not
    // the protocol's: the operator has to be able to see it exists.
    assert_eq!(sections[1].title, "Music");
}

#[tokio::test]
async fn the_item_listing_carries_the_operator_suffixes_plex_expresses_filters_in() {
    let fixture = FixtureServer::answering(
        r#"{"MediaContainer":{"size":1,"totalSize":1200,"Metadata":[
            {"ratingKey":"1001","guid":"plex://movie/1001","type":"movie","title":"Alien",
             "titleSort":"Alien","year":1979,"addedAt":1700000000,
             "thumb":"/library/metadata/1001/thumb/17"}]}}"#,
    )
    .await;

    let query = ItemQuery::new(Window {
        start: 200,
        size: 50,
    })
    .of_type(ItemKind::Movie)
    .sorted_by("addedAt:desc")
    .filtered_by(FilterArgument::new(
        "year",
        FilterOperator::AtLeast,
        vec!["1979".to_owned()],
    ))
    .filtered_by(FilterArgument::new(
        "contentRating",
        FilterOperator::NotEquals,
        vec!["G".to_owned()],
    ))
    .filtered_by(FilterArgument::new(
        "title",
        FilterOperator::ExactEquals,
        vec!["Alien".to_owned()],
    ))
    .filtered_by(FilterArgument::new(
        "genre",
        FilterOperator::All,
        vec!["93".to_owned(), "94".to_owned()],
    ));
    let page = fixture
        .client()
        .items(&section(), &query)
        .await
        .expect("the fixture answers");

    let request = fixture.only_request();
    assert_eq!(request.path, "/library/sections/1/all");
    assert_eq!(request.param("type").as_deref(), Some("1"));
    assert_eq!(request.param("year>>").as_deref(), Some("1979"));
    assert_eq!(request.param("contentRating!").as_deref(), Some("G"));
    assert_eq!(request.param("title=").as_deref(), Some("Alien"));
    assert_eq!(request.params("genre&"), ["93", "94"]);
    assert_eq!(
        request.param("X-Plex-Container-Start").as_deref(),
        Some("200")
    );
    assert_eq!(
        request.param("X-Plex-Container-Size").as_deref(),
        Some("50")
    );

    assert_eq!(page.total, Some(1200));
    assert_eq!(page.items[0].rating_key, RatingKey::new("1001"));
    assert_eq!(page.items[0].scan, ScanState::Complete);
}

#[tokio::test]
async fn one_items_media_facts_read_through_to_streams() {
    let fixture = FixtureServer::answering(
        r#"{"MediaContainer":{"size":1,"Metadata":[
            {"ratingKey":"1001","type":"movie","title":"Alien",
             "Media":[{"container":"mkv","videoResolution":"4k","audioChannels":8,
               "Part":[{"file":"/data/Alien.mkv","accessible":true,"exists":true,
                 "Stream":[{"streamType":2,"codec":"truehd","channels":8,
                   "audioChannelLayout":"7.1","languageCode":"eng"}]}]}]}]}}"#,
    )
    .await;

    let item = fixture
        .client()
        .item(&RatingKey::new("1001"))
        .await
        .expect("the fixture answers");

    assert_eq!(fixture.only_request().path, "/library/metadata/1001");
    let media = item.media().expect("the scan is complete");
    assert_eq!(media[0].video_resolution.as_deref(), Some("4k"));
    assert_eq!(media[0].parts[0].exists, Some(true));
    assert_eq!(
        media[0].parts[0].streams[0].audio_channel_layout.as_deref(),
        Some("7.1")
    );
}

#[tokio::test]
async fn a_collection_is_created_against_the_named_server_and_its_items() {
    let fixture = FixtureServer::answering(
        r#"{"MediaContainer":{"size":1,"Metadata":[
            {"ratingKey":"5001","type":"collection","title":"Best of 1979",
             "childCount":"2","smart":"0"}]}}"#,
    )
    .await;

    let created = fixture
        .client()
        .create_collection(
            &section(),
            ItemKind::Movie,
            "Best of 1979",
            &server(),
            &[RatingKey::new("1001"), RatingKey::new("1002")],
        )
        .await
        .expect("the fixture answers");

    let request = fixture.only_request();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/library/collections");
    assert_eq!(request.param("sectionId").as_deref(), Some("1"));
    // The *members'* type, not the collection's: creation names what is going
    // in, and the edit call below names the collection itself as type 18.
    assert_eq!(request.param("type").as_deref(), Some("1"));
    assert_eq!(
        request.param("uri"),
        library_uri(&server(), &[RatingKey::new("1001"), RatingKey::new("1002")])
    );
    assert_eq!(created.rating_key, RatingKey::new("5001"));
    assert_eq!(created.child_count, Some(2));
}

#[tokio::test]
async fn a_collection_edit_writes_the_sort_title_and_its_lock_together() {
    let fixture = FixtureServer::answering(r#"{"MediaContainer":{"size":1}}"#).await;

    fixture
        .client()
        .edit_collection(
            &section(),
            &RatingKey::new("5001"),
            &CollectionEdit {
                title: Some("Best of 1979".to_owned()),
                sort_title: Some(("!001 Best of 1979".to_owned(), false)),
                mode: Some(CollectionMode::HideItems),
                ..CollectionEdit::default()
            },
        )
        .await
        .expect("the fixture answers");

    let request = fixture.only_request();
    assert_eq!(request.method, "PUT");
    assert_eq!(request.path, "/library/sections/1/all");
    assert_eq!(request.param("id").as_deref(), Some("5001"));
    assert_eq!(request.param("type").as_deref(), Some("18"));
    assert_eq!(
        request.param("titleSort.value").as_deref(),
        Some("!001 Best of 1979")
    );
    // The lock travels with the value in the same request (`I-REV-3`).
    assert_eq!(request.param("titleSort.locked").as_deref(), Some("0"));
    assert_eq!(request.param("collectionMode").as_deref(), Some("1"));
}

#[tokio::test]
async fn collection_items_are_added_removed_and_reordered_by_their_own_endpoints() {
    let add = FixtureServer::answering(r#"{"MediaContainer":{"size":1}}"#).await;
    add.client()
        .add_collection_items(
            &RatingKey::new("5001"),
            &server(),
            &[RatingKey::new("1003")],
        )
        .await
        .expect("the fixture answers");
    let request = add.only_request();
    assert_eq!(request.method, "PUT");
    assert_eq!(request.path, "/library/collections/5001/items");
    assert!(
        request.param("uri").unwrap_or_default().ends_with("/1003"),
        "{request:?}"
    );

    let remove = FixtureServer::answering(r#"{"MediaContainer":{"size":0}}"#).await;
    remove
        .client()
        .remove_collection_item(&RatingKey::new("5001"), &RatingKey::new("1003"))
        .await
        .expect("the fixture answers");
    let request = remove.only_request();
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.path, "/library/collections/5001/items/1003");

    let reorder = FixtureServer::answering(r#"{"MediaContainer":{"size":0}}"#).await;
    reorder
        .client()
        .move_collection_item(
            &RatingKey::new("5001"),
            &RatingKey::new("1003"),
            &MoveTarget::After(RatingKey::new("1001")),
        )
        .await
        .expect("the fixture answers");
    let request = reorder.only_request();
    assert_eq!(request.method, "PUT");
    assert_eq!(request.path, "/library/collections/5001/items/1003/move");
    assert_eq!(request.param("after").as_deref(), Some("1001"));
}

#[tokio::test]
async fn the_hub_list_tells_a_native_row_from_a_collection_row() {
    let fixture = FixtureServer::answering(
        r#"{"MediaContainer":{"size":2,"Hub":[
            {"hubIdentifier":"home.continue","title":"Continue Watching",
             "promotedToOwnHome":"1","promotedToSharedHome":"1"},
            {"hubIdentifier":"collection.5001","title":"Best of 1979","ratingKey":"5001",
             "promotedToOwnHome":"1","promotedToRecommended":"1"}]}}"#,
    )
    .await;

    let listing = fixture
        .client()
        .hubs(&section())
        .await
        .expect("the fixture answers");

    assert_eq!(fixture.only_request().path, "/hubs/sections/1/manage");
    assert_eq!(listing.hubs.len(), 2);
    assert!(
        listing.hubs[0].rating_key.is_none(),
        "a native row is an anchor"
    );
    assert_eq!(listing.hubs[1].rating_key, Some(RatingKey::new("5001")));
    assert!(listing.hubs[1].visibility.recommended);
}

#[tokio::test]
async fn a_hub_is_repositioned_and_its_three_visibility_axes_written_separately() {
    let moved = FixtureServer::answering(r#"{"MediaContainer":{"size":0}}"#).await;
    moved
        .client()
        .move_hub(
            &section(),
            &HubIdentifier::new("collection.5001"),
            &HubMove::After(HubIdentifier::new("home.continue")),
        )
        .await
        .expect("the fixture answers");
    let request = moved.only_request();
    assert_eq!(request.method, "PUT");
    assert_eq!(request.path, "/hubs/sections/1/manage/collection.5001/move");
    assert_eq!(request.param("after").as_deref(), Some("home.continue"));

    let visibility = FixtureServer::answering(r#"{"MediaContainer":{"size":1}}"#).await;
    visibility
        .client()
        .set_hub_visibility(
            &section(),
            &HubIdentifier::new("collection.5001"),
            HubVisibility {
                own_home: true,
                shared_home: false,
                recommended: true,
            },
        )
        .await
        .expect("the fixture answers");
    let request = visibility.only_request();
    assert_eq!(request.path, "/hubs/sections/1/manage/collection.5001");
    assert_eq!(request.param("promotedToOwnHome").as_deref(), Some("1"));
    assert_eq!(request.param("promotedToSharedHome").as_deref(), Some("0"));
    assert_eq!(request.param("promotedToRecommended").as_deref(), Some("1"));
}

#[tokio::test]
async fn a_label_edit_adds_and_removes_in_one_request_and_leaves_the_field_unlocked() {
    let fixture = FixtureServer::answering(r#"{"MediaContainer":{"size":1}}"#).await;

    fixture
        .client()
        .edit_labels(
            &section(),
            ItemKind::Movie,
            &RatingKey::new("1001"),
            &LabelEdit {
                add: vec!["afisharr".to_owned()],
                remove: vec!["old".to_owned()],
            },
        )
        .await
        .expect("the fixture answers");

    let request = fixture.only_request();
    assert_eq!(request.method, "PUT");
    assert_eq!(request.path, "/library/sections/1/all");
    assert_eq!(
        request.param("label[0].tag.tag").as_deref(),
        Some("afisharr")
    );
    assert_eq!(request.param("label[].tag.tag-").as_deref(), Some("old"));
    assert_eq!(request.param("label.locked").as_deref(), Some("0"));
}

#[tokio::test]
async fn a_poster_is_uploaded_as_bytes_under_its_own_declared_type() {
    let fixture = FixtureServer::answering(r#"{"MediaContainer":{"size":1}}"#).await;

    let bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    fixture
        .client()
        .upload_poster(&RatingKey::new("1001"), ArtworkUpload::png(bytes.clone()))
        .await
        .expect("the fixture answers");

    let request = fixture.only_request();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/library/metadata/1001/posters");
    assert_eq!(request.content_type.as_deref(), Some("image/png"));
    assert_eq!(request.body_len, bytes.len());
}

#[tokio::test]
async fn discovery_reads_the_servers_own_fields_operators_and_choices() {
    let vocabulary_fixture = FixtureServer::answering(
        r#"{"MediaContainer":{"size":0,"Meta":{
            "Type":[{"type":"movie","Filter":[
                {"filter":"genre","filterType":"string","title":"Genre",
                 "key":"/library/sections/1/genre?type=1"}],
              "Sort":[{"key":"titleSort","defaultDirection":"asc"}],
              "Field":[{"key":"userRating","type":"integer","subType":"rating"}]}],
            "FieldType":[{"type":"integer","Operator":[{"key":">>=","title":"is at least"}]}]}}}"#,
    )
    .await;

    let vocabulary = vocabulary_fixture
        .client()
        .vocabulary(&section(), ItemKind::Movie)
        .await
        .expect("the fixture answers");

    let request = vocabulary_fixture.only_request();
    assert_eq!(request.path, "/library/sections/1/all");
    assert_eq!(request.param("includeMeta").as_deref(), Some("1"));
    // Zero items: the vocabulary rides alongside a result set nobody wants, and
    // fetching a library to read a field list would put `I-PERF-1` at risk.
    assert_eq!(request.param("X-Plex-Container-Size").as_deref(), Some("0"));
    assert_eq!(
        vocabulary.operators_for("integer").expect("described")[0].key,
        ">>="
    );
    assert_eq!(
        vocabulary.types[0].fields[0].sub_type.as_deref(),
        Some("rating")
    );

    let choices_fixture = FixtureServer::answering(
        r#"{"MediaContainer":{"size":1,"Directory":[
            {"key":"93","title":"Comedy","fastKey":"/library/sections/1/all?genre=93"}]}}"#,
    )
    .await;
    let choices = choices_fixture
        .client()
        .filter_choices(&DiscoveredFilter {
            filter: "genre".to_owned(),
            filter_type: "string".to_owned(),
            title: None,
            key: Some("/library/sections/1/genre?type=1".to_owned()),
        })
        .await
        .expect("the fixture answers");

    let request = choices_fixture.only_request();
    assert_eq!(request.path, "/library/sections/1/genre");
    assert_eq!(request.param("type").as_deref(), Some("1"));
    assert_eq!(choices[0].value, "93");
    assert_eq!(
        choices[0].fast_key.as_deref(),
        Some("/library/sections/1/all?genre=93")
    );
}

#[tokio::test]
async fn a_collection_list_reads_the_librarys_collections() {
    let fixture = FixtureServer::answering(
        r#"{"MediaContainer":{"size":1,"Metadata":[
            {"ratingKey":"5001","type":"collection","title":"Best of 1979",
             "titleSort":"!001 Best of 1979","childCount":"3","smart":"0",
             "collectionSort":"2"}]}}"#,
    )
    .await;

    let collections = fixture
        .client()
        .collections(&section())
        .await
        .expect("the fixture answers");

    assert_eq!(
        fixture.only_request().path,
        "/library/sections/1/collections"
    );
    assert_eq!(collections[0].title, "Best of 1979");
    assert_eq!(collections[0].sort_title.value(), Some("!001 Best of 1979"));
}

#[tokio::test]
async fn a_collections_own_items_are_read_in_the_order_the_server_holds_them() {
    let fixture = FixtureServer::answering(
        r#"{"MediaContainer":{"size":2,"totalSize":2,"Metadata":[
            {"ratingKey":"1002","type":"movie","title":"Aliens"},
            {"ratingKey":"1001","type":"movie","title":"Alien"}]}}"#,
    )
    .await;

    let page = fixture
        .client()
        .collection_items(&RatingKey::new("5001"), &ItemQuery::new(Window::first(200)))
        .await
        .expect("the fixture answers");

    assert_eq!(
        fixture.only_request().path,
        "/library/collections/5001/children"
    );
    // The order is the answer: a client that sorted it could not see the
    // silent no-op move §15.3 describes.
    assert_eq!(
        page.items
            .iter()
            .map(|item| item.rating_key.to_string())
            .collect::<Vec<_>>(),
        ["1002", "1001"]
    );
}

#[tokio::test]
async fn a_collection_is_deleted_by_its_own_key() {
    let fixture = FixtureServer::answering(r#"{"MediaContainer":{"size":0}}"#).await;
    fixture
        .client()
        .delete_collection(&RatingKey::new("5001"))
        .await
        .expect("the fixture answers");
    let request = fixture.only_request();
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.path, "/library/collections/5001");
}

#[tokio::test]
async fn a_refusal_is_reported_as_an_answer_and_a_status() {
    // The distinction `I-SRC-1` is built on: 401 means the token is no longer
    // accepted, and a caller that could not tell it from an outage would retry
    // for ever against a credential that will never work again.
    let fixture = FixtureServer::answering_with(401, r#"{"error":"unauthorized"}"#).await;

    let error = fixture
        .client()
        .sections()
        .await
        .expect_err("a 401 is not a section list");

    assert!(error.server_answered());
    assert_eq!(error.refused_status(), Some(401));
}
