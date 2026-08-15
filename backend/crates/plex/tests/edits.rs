// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Every write this build makes, against a hand-rolled fixture response.
//!
//! The half of the surface that changes somebody's library, so the request
//! assertion is the point: the method, the path, and the exact argument shapes
//! a real Plex reads. A comma-joined label removal sent as a repeated key
//! removes one label and reports success for two, and only the request
//! assertion can see it.

mod fixtures;

use afisharr_plex::{
    artwork::ArtworkUpload,
    collections::{CollectionEdit, CollectionMode, MoveTarget, library_uri},
    hubs::{HubIdentifier, HubListing, HubMove, HubVisibility},
    labels::LabelEdit,
    libraries::{ItemKind, RatingKey, SectionKey},
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
async fn a_collection_with_no_row_yet_is_promoted_rather_than_written_to() {
    // A `PUT` to `/manage/{identifier}` addresses a row a real server does not
    // have until something promotes the collection: it answers 200 and changes
    // nothing, which is a promotion path that never promotes.
    let fixture = FixtureServer::answering(r#"{"MediaContainer":{"size":0}}"#).await;
    let empty = HubListing {
        hubs: Vec::new(),
        unidentifiable: 0,
    };

    fixture
        .client()
        .set_collection_visibility(
            &section(),
            &empty,
            &RatingKey::new("5001"),
            HubVisibility {
                own_home: true,
                shared_home: false,
                recommended: false,
            },
        )
        .await
        .expect("the fixture answers");

    let request = fixture.only_request();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/hubs/sections/1/manage");
    assert_eq!(request.param("metadataItemId").as_deref(), Some("5001"));
    assert_eq!(request.param("promotedToOwnHome").as_deref(), Some("1"));
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
async fn two_label_removals_travel_as_one_comma_joined_argument() {
    // A real server reads one argument holding every removed tag, quoted
    // individually and joined with commas. Sent as a repeated key, the second
    // removal overwrote the first and the call reported success for both.
    let fixture = FixtureServer::answering(r#"{"MediaContainer":{"size":1}}"#).await;

    fixture
        .client()
        .edit_labels(
            &section(),
            ItemKind::Movie,
            &RatingKey::new("1001"),
            &LabelEdit {
                add: Vec::new(),
                remove: vec!["old".to_owned(), "a,b".to_owned()],
            },
        )
        .await
        .expect("the fixture answers");

    let request = fixture.only_request();
    assert_eq!(
        request.param("label[].tag.tag-").as_deref(),
        Some("old,a%2Cb"),
        "the comma inside a label is quoted so the join stays unambiguous"
    );
    assert_eq!(request.params("label[].tag.tag-").len(), 1);
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
