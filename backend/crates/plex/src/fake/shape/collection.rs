// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One collection, in the shape a collection row takes.

use crate::fake::{
    element::Element,
    state::{FakeCollection, FakeLibrary},
};

/// One collection, in the shape a collection row takes.
///
/// `key` is `/library/metadata/{ratingKey}/children`, which a client strips the
/// suffix off and builds every item call from (`plexapi/collection.py:78`) —
/// items, item removal, item moves, and the delete. A collection answered
/// without it has no addressable items at all.
pub(crate) fn collection(collection: &FakeCollection, library: &FakeLibrary) -> Element {
    let mut row = Element::content("Directory")
        .text("ratingKey", collection.rating_key.clone())
        .text(
            "key",
            format!("/library/metadata/{}/children", collection.rating_key),
        )
        .text("guid", format!("collection://{}", collection.rating_key))
        .text("type", "collection")
        // The libtype of what is inside it. A client refuses to add an item of
        // another type on the strength of this (`plexapi/collection.py:326`),
        // and a collection that did not state it refused everything.
        .text("subtype", collection.subtype.clone())
        .text("title", collection.title.clone())
        .maybe_text("titleSort", collection.sort_title.clone())
        .maybe_text("summary", collection.summary.clone())
        .text("librarySectionID", library.key.clone())
        .text("librarySectionTitle", library.title.clone())
        .text("librarySectionUUID", library.uuid.clone())
        .number(
            "childCount",
            i64::try_from(collection.items.len()).unwrap_or(i64::MAX),
        )
        .flag("smart", collection.smart)
        .number("collectionMode", i64::from(collection.mode))
        .number("collectionSort", i64::from(collection.sort))
        .number("addedAt", 1_700_000_000_i64)
        .number("updatedAt", 1_700_000_100_i64);

    if collection.sort_title_locked {
        row = row.child(
            Element::named("Field")
                .text("name", "titleSort")
                .flag("locked", true),
        );
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{json, library::World, scenario::Scenario, xml};

    fn world() -> World {
        World::build(&Scenario::behaving(1))
    }

    fn rendered() -> serde_json::Value {
        let world = world();
        let library = &world.libraries[0];
        json::document(&collection(&library.collections[0], library))
    }

    #[test]
    fn a_collection_is_addressed_by_the_key_its_items_hang_off() {
        assert_eq!(
            rendered()["Metadata"]["key"],
            "/library/metadata/15001/children"
        );
    }

    #[test]
    fn a_collection_knows_which_section_it_is_in() {
        // Without it a client asks `/hubs/sections/None/manage` for its
        // ordering-space row, and reads the 404 as "not promoted".
        assert_eq!(rendered()["Metadata"]["librarySectionID"], "1");
    }

    #[test]
    fn a_new_collection_reports_release_order_rather_than_custom() {
        // Custom order is a thing Afisharr must switch on. A fake that started
        // there tests nothing (`plexapi/collection.py:73`).
        assert_eq!(rendered()["Metadata"]["collectionSort"], 0);
    }

    #[test]
    fn the_counts_and_flags_are_spelled_one_way() {
        // `childCount` as a string and `smart` as a string were two spellings
        // of two facts that arrive as XML attributes side by side.
        assert_eq!(rendered()["Metadata"]["childCount"], 3);
        assert_eq!(rendered()["Metadata"]["smart"], 0);
    }

    #[test]
    fn a_collection_is_a_directory_in_xml_and_a_metadata_row_in_json() {
        let world = world();
        let library = &world.libraries[0];
        let document = xml::document(&collection(&library.collections[0], library));
        assert!(
            document.contains("<Directory ") && document.contains("type=\"collection\""),
            "{document}"
        );
        assert!(rendered().get("Metadata").is_some());
    }
}
