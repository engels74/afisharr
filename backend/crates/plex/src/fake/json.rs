// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The JSON shapes the fake answers in.
//!
//! Written out by hand rather than serialised from the client's own types, and
//! deliberately: a fake that answered by re-serialising the structs the client
//! parses would agree with the client by construction and prove nothing. These
//! are the shapes a real server sends, and the contract test in `tests/` is
//! what keeps that claim honest.

use serde_json::{Value, json};

use crate::fake::state::{FakeCollection, FakeHub, FakeItem, FakeLibrary};

/// Wraps a body in the envelope every Plex answer arrives in.
pub(crate) fn container(body: &Value) -> Value {
    json!({ "MediaContainer": body })
}

/// One item, in the shape `Metadata` entries take.
pub(crate) fn item(item: &FakeItem) -> Value {
    let mut body = json!({
        "ratingKey": item.rating_key,
        "guid": item.guid,
        "type": item.kind,
        "title": item.title,
        "addedAt": 1_700_000_000,
        "updatedAt": 1_700_000_100,
        "thumb": item.thumb,
    });
    let object = body.as_object_mut().expect("the body is an object");

    // Presence, not emptiness: an absent sort title is a missing attribute, and
    // an empty string is a value. §15.6 turns on the difference.
    if let Some(sort_title) = &item.sort_title {
        object.insert("titleSort".to_owned(), json!(sort_title));
    }
    if let Some(year) = item.year {
        object.insert("year".to_owned(), json!(year));
    }
    if item.sort_title_locked {
        object.insert(
            "Field".to_owned(),
            json!([{ "name": "titleSort", "locked": true }]),
        );
    }
    if !item.labels.is_empty() {
        let labels: Vec<Value> = item
            .labels
            .iter()
            .map(|tag| json!({ "tag": tag }))
            .collect();
        object.insert("Label".to_owned(), json!(labels));
    }
    if item.indexed {
        if item.has_media {
            object.insert("Media".to_owned(), media(item));
        }
    } else {
        // Still indexing: no media, and the flag that says why. Without the
        // flag this is a film with no file, which is a different fact (P1).
        object.insert("refreshing".to_owned(), json!(true));
    }
    body
}

/// One item's media, parts, and streams.
fn media(item: &FakeItem) -> Value {
    json!([{
        "id": 1,
        "container": "mkv",
        "videoResolution": "1080",
        "videoCodec": "h264",
        "audioCodec": "eac3",
        "audioChannels": 6,
        "bitrate": 8000,
        "width": 1920,
        "height": 1080,
        "duration": 7_200_000,
        "Part": [{
            "id": 1,
            "file": format!("/data/{}.mkv", item.rating_key),
            "size": 4_000_000_000_u64,
            "container": "mkv",
            "accessible": true,
            "exists": true,
            "Stream": [
                { "streamType": 1, "codec": "h264", "bitDepth": 8, "colorSpace": "bt709" },
                {
                    "streamType": 2, "codec": "eac3", "channels": 6,
                    "audioChannelLayout": "5.1", "language": "English", "languageCode": "eng"
                },
                {
                    "streamType": 3, "codec": "subrip", "language": "English",
                    "languageCode": "eng", "forced": false
                }
            ]
        }]
    }])
}

/// One collection, in the shape a collection list takes.
pub(crate) fn collection(collection: &FakeCollection) -> Value {
    let mut body = json!({
        "ratingKey": collection.rating_key,
        "type": "collection",
        "title": collection.title,
        "childCount": collection.items.len().to_string(),
        "smart": "0",
        "collectionSort": "2",
    });
    let object = body.as_object_mut().expect("the body is an object");
    if let Some(sort_title) = &collection.sort_title {
        object.insert("titleSort".to_owned(), json!(sort_title));
    }
    if collection.sort_title_locked {
        object.insert(
            "Field".to_owned(),
            json!([{ "name": "titleSort", "locked": true }]),
        );
    }
    body
}

/// One hub, in the shape the manage endpoint answers with.
pub(crate) fn hub(hub: &FakeHub) -> Value {
    let mut body = json!({
        "hubIdentifier": hub.identifier,
        "title": hub.title,
        "promotedToOwnHome": i32::from(hub.own_home).to_string(),
        "promotedToSharedHome": i32::from(hub.shared_home).to_string(),
        "promotedToRecommended": i32::from(hub.recommended).to_string(),
    });
    if let Some(rating_key) = &hub.rating_key {
        body.as_object_mut()
            .expect("the body is an object")
            .insert("ratingKey".to_owned(), json!(rating_key));
    }
    body
}

/// One library, in the shape the section list answers with.
pub(crate) fn section(library: &FakeLibrary) -> Value {
    json!({
        "key": library.key,
        "uuid": library.uuid,
        "type": library.kind,
        "title": library.title,
        "agent": "tv.plex.agents.movie",
        "language": "en-US",
        "scannedAt": 1_700_000_000,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> FakeItem {
        FakeItem {
            rating_key: "1001".to_owned(),
            guid: "plex://movie/1001".to_owned(),
            kind: "movie".to_owned(),
            title: "Film 1".to_owned(),
            sort_title: Some("Film 1".to_owned()),
            sort_title_locked: false,
            year: Some(1979),
            thumb: "/library/metadata/1001/thumb/17".to_owned(),
            indexed: true,
            has_media: true,
            labels: vec!["afisharr".to_owned()],
        }
    }

    #[test]
    fn an_indexed_item_carries_its_media_and_no_refresh_flag() {
        let body = item(&base());
        assert!(body.get("Media").is_some());
        assert!(body.get("refreshing").is_none());
        assert_eq!(body["Label"][0]["tag"], "afisharr");
    }

    #[test]
    fn an_item_still_indexing_carries_the_flag_and_no_media_at_all() {
        let body = item(&FakeItem {
            indexed: false,
            has_media: false,
            ..base()
        });
        assert_eq!(body["refreshing"], true);
        assert!(body.get("Media").is_none());
    }

    #[test]
    fn an_absent_sort_title_is_a_missing_attribute_and_not_an_empty_one() {
        let body = item(&FakeItem {
            sort_title: None,
            ..base()
        });
        assert!(body.get("titleSort").is_none());
    }

    #[test]
    fn a_locked_sort_title_is_reported_in_the_field_list() {
        let body = item(&FakeItem {
            sort_title_locked: true,
            ..base()
        });
        assert_eq!(body["Field"][0]["name"], "titleSort");
        assert_eq!(body["Field"][0]["locked"], true);
    }

    #[test]
    fn a_sort_title_can_be_absent_and_locked_at_the_same_time() {
        // The state a restore gets wrong, and the reason §15.6 names three
        // properties rather than one.
        let body = item(&FakeItem {
            sort_title: None,
            sort_title_locked: true,
            ..base()
        });
        assert!(body.get("titleSort").is_none());
        assert_eq!(body["Field"][0]["locked"], true);
    }

    #[test]
    fn a_hub_with_no_collection_behind_it_carries_no_rating_key() {
        let native = hub(&FakeHub {
            identifier: "home.continue".to_owned(),
            title: "Continue Watching".to_owned(),
            rating_key: None,
            own_home: true,
            shared_home: false,
            recommended: false,
        });
        assert!(native.get("ratingKey").is_none());
        assert_eq!(native["promotedToOwnHome"], "1");
        assert_eq!(native["promotedToSharedHome"], "0");
    }
}
