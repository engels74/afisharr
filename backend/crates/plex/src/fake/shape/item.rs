// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One library item, in the shape a content row takes.

use crate::fake::{
    element::Element,
    shape::{Detail, media::media},
    state::{FakeItem, FakeLibrary},
};

/// What a real server calls a content row of this kind, in XML.
///
/// Plex has four element names here and its JSON translation collapses all of
/// them to `Metadata`. A fake that emitted the JSON name in XML would be
/// unreadable by a client that resolves its classes from the tag, which is
/// every XML client there is.
const fn content_tag(kind: &str) -> &'static str {
    match kind.as_bytes() {
        b"show" | b"season" | b"artist" | b"album" | b"photoalbum" => "Directory",
        b"track" => "Track",
        b"photo" => "Photo",
        _ => "Video",
    }
}

/// Whether a row of this kind is addressed as a container of other rows.
const fn is_container(kind: &str) -> bool {
    matches!(
        kind.as_bytes(),
        b"show" | b"season" | b"artist" | b"album" | b"photoalbum"
    )
}

/// One item, in the shape a content row takes.
///
/// The `librarySection*` attributes are on the row as well as on the container
/// it arrives in, because a client that reloads one item reads the row alone:
/// the container's copy is lost, the item no longer knows its own section, and
/// every call that starts from the section fails on a `None` (P1).
pub(crate) fn item(item: &FakeItem, library: &FakeLibrary, detail: Detail) -> Element {
    let key = if is_container(&item.kind) {
        format!("/library/metadata/{}/children", item.rating_key)
    } else {
        format!("/library/metadata/{}", item.rating_key)
    };
    let mut row = Element::content(content_tag(&item.kind))
        .text("ratingKey", item.rating_key.clone())
        .text("key", key)
        .text("guid", item.guid.clone())
        .text("type", item.kind.clone())
        .text("title", item.title.clone())
        // Presence, not emptiness: an absent sort title is a missing attribute,
        // and an empty string is a value. §15.6 turns on the difference.
        .maybe_text("titleSort", item.sort_title.clone())
        .maybe_number("year", item.year)
        .maybe_number("index", item.index)
        .maybe_text("parentRatingKey", item.parent_rating_key.clone())
        .maybe_text(
            "originallyAvailableAt",
            item.originally_available_at.clone(),
        )
        .number("addedAt", 1_700_000_000_i64)
        .number("updatedAt", 1_700_000_100_i64)
        .text("thumb", item.thumb.clone())
        .text("librarySectionID", library.key.clone())
        .text("librarySectionTitle", library.title.clone())
        .text(
            "librarySectionKey",
            format!("/library/sections/{}", library.key),
        );

    if item.sort_title_locked {
        row = row.child(
            Element::named("Field")
                .text("name", "titleSort")
                .flag("locked", true),
        );
    }
    if item.labels_locked {
        row = row.child(
            Element::named("Field")
                .text("name", "label")
                .flag("locked", true),
        );
    }
    row = row.children(
        item.genres
            .iter()
            .map(|genre| Element::named("Genre").text("tag", genre.clone())),
    );
    row = row.children(
        item.labels
            .iter()
            .map(|label| Element::named("Label").text("tag", label.clone())),
    );
    // The external ids a resolver matches on, and only when the request asked
    // for them. A reference client sends `includeGuids=1` on every listing and
    // every detail fetch (`plexapi/library.py:1266`, `plexapi/base.py:209`), so
    // there is no evidence a server answers them unasked — and a fake that did
    // would let a client that never sends the argument read external ids here
    // and none at all from a real Plex.
    if detail.include_guids {
        row = row.children(
            item.external_guids
                .iter()
                .map(|guid| Element::named("Guid").text("id", guid.clone())),
        );
    }

    if item.indexed {
        if item.has_media {
            row = row.child(media(item, detail));
        }
    } else {
        // Still indexing: no media, and the flag that says why. Without the
        // flag this is a film with no file, which is a different fact (P1).
        row = row.flag("refreshing", true);
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{json, xml};

    fn base() -> FakeItem {
        FakeItem {
            rating_key: "1001".to_owned(),
            guid: "plex://movie/1001".to_owned(),
            external_guids: vec!["imdb://tt0078748".to_owned()],
            kind: "movie".to_owned(),
            title: "Film 1".to_owned(),
            sort_title: Some("Film 1".to_owned()),
            sort_title_locked: false,
            year: Some(1979),
            index: Some(1),
            parent_rating_key: None,
            originally_available_at: Some("1979-05-25".to_owned()),
            thumb: "/library/metadata/1001/thumb/17".to_owned(),
            indexed: true,
            has_media: true,
            genres: vec!["Science Fiction".to_owned()],
            labels: vec!["afisharr".to_owned()],
            labels_locked: false,
        }
    }

    fn library() -> FakeLibrary {
        crate::fake::library::World::build(&crate::fake::scenario::Scenario::behaving(1))
            .libraries
            .swap_remove(0)
    }

    fn rendered(item_body: &FakeItem) -> serde_json::Value {
        json::document(&item(item_body, &library(), Detail::PLAIN))
    }

    #[test]
    fn an_indexed_item_carries_its_media_and_no_refresh_flag() {
        let body = rendered(&base());
        assert!(body["Metadata"].get("Media").is_some());
        assert!(body["Metadata"].get("refreshing").is_none());
        assert_eq!(body["Metadata"]["Label"][0]["tag"], "afisharr");
    }

    #[test]
    fn the_external_ids_are_answered_only_when_the_request_asked_for_them() {
        // Answered unconditionally, the fake hid a client that never sends
        // `includeGuids=1` — it read external ids here and would read none at
        // all from a real server, which is the highest-volume lookup in the
        // product resolving against nothing (P1).
        assert!(rendered(&base())["Metadata"].get("Guid").is_none());
        let asked = json::document(&item(
            &base(),
            &library(),
            Detail {
                include_guids: true,
                ..Detail::PLAIN
            },
        ));
        assert_eq!(asked["Metadata"]["Guid"][0]["id"], "imdb://tt0078748");
    }

    #[test]
    fn an_item_still_indexing_carries_the_flag_and_no_media_at_all() {
        let body = rendered(&FakeItem {
            indexed: false,
            has_media: false,
            ..base()
        });
        assert_eq!(body["Metadata"]["refreshing"], 1);
        assert!(body["Metadata"].get("Media").is_none());
    }

    #[test]
    fn an_absent_sort_title_is_a_missing_attribute_and_not_an_empty_one() {
        let body = rendered(&FakeItem {
            sort_title: None,
            ..base()
        });
        assert!(body["Metadata"].get("titleSort").is_none());
    }

    #[test]
    fn a_sort_title_can_be_absent_and_locked_at_the_same_time() {
        // The state a restore gets wrong, and the reason §15.6 names three
        // properties rather than one.
        let body = rendered(&FakeItem {
            sort_title: None,
            sort_title_locked: true,
            ..base()
        });
        assert!(body["Metadata"].get("titleSort").is_none());
        assert_eq!(body["Metadata"]["Field"][0]["name"], "titleSort");
        assert_eq!(body["Metadata"]["Field"][0]["locked"], 1);
    }

    #[test]
    fn a_locked_label_field_is_reported_the_same_way_a_locked_sort_title_is() {
        let body = rendered(&FakeItem {
            labels_locked: true,
            ..base()
        });
        assert_eq!(body["Metadata"]["Field"][0]["name"], "label");
    }

    #[test]
    fn a_row_is_written_under_the_element_name_its_kind_takes() {
        // Two names for one row, and only in XML: a movie is `<Video>` and a
        // show is `<Directory>`, and both are `Metadata` in JSON.
        let movie = xml::document(&item(&base(), &library(), Detail::PLAIN));
        assert!(movie.contains("<Video "), "{movie}");
        let show = xml::document(&item(
            &FakeItem {
                kind: "show".to_owned(),
                ..base()
            },
            &library(),
            Detail::PLAIN,
        ));
        assert!(show.contains("<Directory "), "{show}");
        assert!(rendered(&base()).get("Metadata").is_some());
    }

    #[test]
    fn a_container_row_is_keyed_at_its_children_and_a_leaf_row_at_itself() {
        assert_eq!(
            rendered(&base())["Metadata"]["key"],
            "/library/metadata/1001"
        );
        let show = rendered(&FakeItem {
            kind: "show".to_owned(),
            ..base()
        });
        assert_eq!(show["Metadata"]["key"], "/library/metadata/1001/children");
    }
}
