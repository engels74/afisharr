// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Building one library's items, collections, and ordering space from a seed.

use crate::fake::{
    scenario::{LibrarySpec, Scenario, UNRECOGNISED_ARTWORK},
    seed::Seed,
    state::{FakeCollection, FakeHub, FakeItem, FakeLibrary},
    vocabulary::GENRES,
};

/// The identifier a real server gives the ordering-space row of a collection.
///
/// The shape `python-plexapi` synthesises for a collection that has never been
/// promoted (`plexapi/collection.py:213`), which is the only evidence in reach
/// of what a promoted one is called: the last dot-segment is the rating key,
/// and it is what the promotion call sends as `metadataItemId`.
pub(crate) fn hub_identifier(section: &str, collection: &str) -> String {
    format!("custom.collection.{section}.{collection}")
}

/// Builds one library from its declaration and the shared seed stream.
pub(crate) fn library(scenario: &Scenario, seed: &mut Seed, spec: &LibrarySpec) -> FakeLibrary {
    let items: Vec<FakeItem> = (0..spec.items)
        .map(|index| item(scenario, seed, spec, index))
        .collect();
    let collection = FakeCollection {
        rating_key: format!("{}5001", spec.key),
        title: format!("{} Collection", spec.title),
        sort_title: None,
        sort_title_locked: false,
        summary: None,
        subtype: spec.kind.clone(),
        mode: -1,
        // Release order, which is where a real server starts a new collection
        // (`plexapi/collection.py:73`). Custom order is a thing Afisharr must
        // switch on, and a fake that started there tests nothing.
        sort: 0,
        smart: spec.smart_collection,
        items: items
            .iter()
            .take(3)
            .map(|item| item.rating_key.clone())
            .collect(),
        moves_left: scenario.move_budget,
    };
    let hubs = vec![
        // A native row first: it cannot be removed or unpromoted, so every plan
        // the ordering tests write has an anchor to work around (§15.1).
        FakeHub {
            identifier: format!("home.continue.{}", spec.key),
            title: "Continue Watching".to_owned(),
            rating_key: None,
            deletable: false,
            own_home: true,
            shared_home: true,
            recommended: false,
        },
        FakeHub {
            identifier: hub_identifier(&spec.key, &collection.rating_key),
            title: collection.title.clone(),
            rating_key: Some(collection.rating_key.clone()),
            deletable: true,
            own_home: true,
            shared_home: false,
            recommended: true,
        },
    ];
    FakeLibrary {
        key: spec.key.clone(),
        uuid: format!("uuid-section-{}", spec.key),
        kind: spec.kind.clone(),
        title: spec.title.clone(),
        scanner: scanner(&spec.kind).to_owned(),
        locations: vec![format!("/data/{}", spec.kind)],
        items,
        collections: vec![collection],
        hubs,
        hub_moves_left: scenario.move_budget,
    }
}

/// The scanner a real server names for a library of this kind.
const fn scanner(kind: &str) -> &'static str {
    match kind.as_bytes() {
        b"show" => "Plex TV Series",
        b"artist" => "Plex Music",
        b"photo" => "Plex Photo Scanner",
        _ => "Plex Movie",
    }
}

/// What a library of this kind calls one of its items.
fn noun(kind: &str) -> &'static str {
    match kind {
        "show" => "Series",
        "artist" => "Artist",
        "photo" => "Photo",
        _ => "Film",
    }
}

/// Builds one item, drawing each misbehaviour from the seed in a fixed order.
fn item(scenario: &Scenario, seed: &mut Seed, spec: &LibrarySpec, index: u32) -> FakeItem {
    let rating_key = format!("{}{:04}", spec.key, index + 1);
    // Every draw happens for every item, whether or not the scenario asked for
    // the behaviour. Drawing conditionally would make the stream's position
    // depend on the scenario, so two scenarios sharing a seed would disagree
    // about items neither of them changed.
    let unrecognised = seed.one_in(scenario.unrecognised_artwork_every.unwrap_or(0));
    let indexing = seed.one_in(scenario.partial_scan_every.unwrap_or(0));
    let absent_sort = seed.one_in(scenario.absent_sort_title_every.unwrap_or(0));
    let locked_sort = seed.one_in(scenario.locked_sort_title_every.unwrap_or(0));
    let format_choice = seed.below(u64::try_from(UNRECOGNISED_ARTWORK.len()).unwrap_or(1));

    let title = format!("{} {}", noun(&spec.kind), index + 1);
    let thumb = if unrecognised {
        let template = UNRECOGNISED_ARTWORK[usize::try_from(format_choice).unwrap_or(0)];
        template.replace("{key}", &rating_key)
    } else {
        format!("/library/metadata/{rating_key}/thumb/1700000000")
    };
    let year = 1980 + i32::try_from(index % 40).unwrap_or(0);

    FakeItem {
        guid: format!("plex://{}/{rating_key}", spec.kind),
        // The external ids a resolver matches on, which the client parses and
        // the fake never sent — so the parser was checked against nothing.
        external_guids: vec![
            format!("imdb://tt{:07}", index + 1),
            format!("tmdb://{}", 1000 + index),
        ],
        kind: spec.kind.clone(),
        sort_title: if absent_sort {
            None
        } else {
            Some(title.clone())
        },
        sort_title_locked: locked_sort,
        year: Some(year),
        index: Some(i32::try_from(index + 1).unwrap_or(1)),
        parent_rating_key: None,
        originally_available_at: Some(format!("{year}-05-25")),
        thumb,
        indexed: !indexing,
        has_media: !indexing,
        // Assigned round-robin rather than drawn, so a filter test can say what
        // the right answer is without replaying the seed.
        genres: vec![
            GENRES[usize::try_from(index).unwrap_or(0) % GENRES.len()]
                .1
                .to_owned(),
        ],
        labels: Vec::new(),
        labels_locked: false,
        title,
        rating_key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::library::World;

    #[test]
    fn asking_for_unrecognised_artwork_serves_more_than_one_shape() {
        let world = World::build(
            &Scenario::behaving(5)
                .holding(200, 0)
                .unrecognised_artwork(2),
        );
        let odd: Vec<&str> = world.libraries[0]
            .items
            .iter()
            .map(|item| item.thumb.as_str())
            .filter(|thumb| !thumb.starts_with("/library/metadata/"))
            .collect();
        assert!(
            odd.iter().any(|thumb| thumb.starts_with("upload://")),
            "{odd:?}"
        );
        assert!(
            odd.iter().any(|thumb| thumb.starts_with("blorp:")),
            "{odd:?}"
        );
    }

    #[test]
    fn every_genre_the_vocabulary_declares_is_reachable_in_the_library() {
        // A filter test asserting "genre 93 returns four items" needs the world
        // to hold items of every declared genre, and to hold them without
        // replaying the seed to find out which.
        let world = World::build(&Scenario::behaving(1).holding(12, 0));
        for (_, title) in GENRES {
            assert!(
                world.libraries[0]
                    .items
                    .iter()
                    .any(|item| item.genres.iter().any(|genre| genre == title)),
                "no item carries {title}"
            );
        }
    }

    #[test]
    fn a_collection_row_is_named_the_way_a_client_addresses_it() {
        // The last dot-segment is the rating key, which is what the promotion
        // call sends as `metadataItemId` (`plexapi/library.py:3115`).
        let world = World::build(&Scenario::behaving(1));
        let row = &world.libraries[0].hubs[1];
        assert_eq!(row.identifier, "custom.collection.1.15001");
        assert!(row.deletable, "a collection row can leave the space");
        assert!(
            !world.libraries[0].hubs[0].deletable,
            "one of Plex's own cannot"
        );
    }

    #[test]
    fn a_new_collection_starts_in_release_order_rather_than_custom() {
        let world = World::build(&Scenario::behaving(1));
        assert_eq!(world.libraries[0].collections[0].sort, 0);
    }

    #[test]
    fn a_library_names_the_scanner_its_kind_uses() {
        // One scanner for every section would have a music library declaring
        // the movie scanner, which is a fact no real server states.
        let world = World::build(&Scenario::behaving(1));
        assert_eq!(world.libraries[0].scanner, "Plex Movie");
        assert_eq!(world.libraries[1].scanner, "Plex TV Series");
    }
}
