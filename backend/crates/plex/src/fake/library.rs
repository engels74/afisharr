// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Building the fake's world from a scenario and its seed.

use crate::fake::{
    scenario::{Scenario, UNRECOGNISED_ARTWORK},
    seed::Seed,
    state::{FakeCollection, FakeHub, FakeItem, FakeLibrary},
};

/// The whole world one run of the fake serves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct World {
    pub(crate) machine_identifier: String,
    pub(crate) version: String,
    pub(crate) friendly_name: String,
    pub(crate) libraries: Vec<FakeLibrary>,
    /// How many times the item list has been read, for rating-key churn.
    pub(crate) fetches: u32,
}

impl World {
    /// Builds the world a scenario describes.
    ///
    /// Every varying decision is drawn from the scenario's own seed and in a
    /// fixed order, so the same seed produces the same world byte for byte —
    /// which is the property D-036 turns into an assertion.
    pub(crate) fn build(scenario: &Scenario) -> Self {
        let mut seed = scenario.seed.rewound();
        let movies = library(scenario, &mut seed, "1", "movie", "Movies", scenario.movies);
        let shows = library(scenario, &mut seed, "2", "show", "TV", scenario.shows);
        Self {
            machine_identifier: scenario.machine_identifier.clone(),
            version: scenario.version.clone(),
            friendly_name: scenario.friendly_name.clone(),
            libraries: vec![movies, shows],
            fetches: 0,
        }
    }

    /// The library with this section key.
    pub(crate) fn library(&mut self, key: &str) -> Option<&mut FakeLibrary> {
        self.libraries.iter_mut().find(|library| library.key == key)
    }

    /// The library holding this collection.
    pub(crate) fn library_of_collection(&mut self, collection: &str) -> Option<&mut FakeLibrary> {
        self.libraries.iter_mut().find(|library| {
            library
                .collections
                .iter()
                .any(|candidate| candidate.rating_key == collection)
        })
    }

    /// Re-keys every item, keeping its guid.
    ///
    /// Rating-key churn (`I-ID-1`): the same logical item comes back under a
    /// new key, and a client that treated the key as identity now has two rows
    /// for one film. The guid is what survives, because it is the identity.
    pub(crate) fn churn_rating_keys(&mut self) {
        for library in &mut self.libraries {
            for item in &mut library.items {
                item.rating_key = format!("{}9", item.rating_key);
            }
            // Membership follows the item, because on a real server it does:
            // the collection holds the item, not the number Plex prints for it.
            // A fake that left the old keys behind would empty every collection
            // on every churn, and a test that then reported the collection as
            // lost would be reporting a failure no Plex produces (PRD §21.10.2).
            for collection in &mut library.collections {
                for key in &mut collection.items {
                    *key = format!("{key}9");
                }
            }
        }
    }
}

/// Builds one library's worth of items, collections, and hubs.
fn library(
    scenario: &Scenario,
    seed: &mut Seed,
    key: &str,
    kind: &str,
    title: &str,
    count: u32,
) -> FakeLibrary {
    let items: Vec<FakeItem> = (0..count)
        .map(|index| item(scenario, seed, key, kind, index))
        .collect();
    let collections = vec![FakeCollection {
        rating_key: format!("{key}5001"),
        title: format!("{title} Collection"),
        sort_title: None,
        sort_title_locked: false,
        items: items
            .iter()
            .take(3)
            .map(|item| item.rating_key.clone())
            .collect(),
    }];
    let hubs = vec![
        // A native row first: it cannot be unpromoted, so every plan the
        // ordering tests write has an anchor to work around (§15.1).
        FakeHub {
            identifier: format!("home.continue.{key}"),
            title: "Continue Watching".to_owned(),
            rating_key: None,
            own_home: true,
            shared_home: true,
            recommended: false,
        },
        FakeHub {
            identifier: format!("collection.{key}5001"),
            title: format!("{title} Collection"),
            rating_key: Some(format!("{key}5001")),
            own_home: true,
            shared_home: false,
            recommended: true,
        },
    ];
    FakeLibrary {
        key: key.to_owned(),
        uuid: format!("uuid-section-{key}"),
        kind: kind.to_owned(),
        title: title.to_owned(),
        items,
        collections,
        hubs,
        moves_left: scenario.move_budget,
    }
}

/// Builds one item, drawing each misbehaviour from the seed in a fixed order.
fn item(scenario: &Scenario, seed: &mut Seed, key: &str, kind: &str, index: u32) -> FakeItem {
    let rating_key = format!("{key}{:04}", index + 1);
    // Every draw happens for every item, whether or not the scenario asked for
    // the behaviour. Drawing conditionally would make the stream's position
    // depend on the scenario, so two scenarios sharing a seed would disagree
    // about items neither of them changed.
    let unrecognised = seed.one_in(scenario.unrecognised_artwork_every.unwrap_or(0));
    let indexing = seed.one_in(scenario.partial_scan_every.unwrap_or(0));
    let absent_sort = seed.one_in(scenario.absent_sort_title_every.unwrap_or(0));
    let locked_sort = seed.one_in(scenario.locked_sort_title_every.unwrap_or(0));
    let format_choice = seed.below(u64::try_from(UNRECOGNISED_ARTWORK.len()).unwrap_or(1));

    let title = format!(
        "{} {}",
        if kind == "movie" { "Film" } else { "Series" },
        index + 1
    );
    let thumb = if unrecognised {
        let template = UNRECOGNISED_ARTWORK[usize::try_from(format_choice).unwrap_or(0)];
        template.replace("{key}", &rating_key)
    } else {
        format!("/library/metadata/{rating_key}/thumb/1700000000")
    };

    FakeItem {
        guid: format!("plex://{kind}/{rating_key}"),
        kind: kind.to_owned(),
        sort_title: if absent_sort {
            None
        } else {
            Some(title.clone())
        },
        sort_title_locked: locked_sort,
        year: Some(1980 + i32::try_from(index % 40).unwrap_or(0)),
        thumb,
        indexed: !indexing,
        has_media: !indexing,
        labels: Vec::new(),
        title,
        rating_key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_builds_the_same_world() {
        let scenario = Scenario::behaving(2024)
            .unrecognised_artwork(3)
            .partially_scanned(4)
            .absent_sort_titles(5)
            .locked_sort_titles(6);
        assert_eq!(World::build(&scenario), World::build(&scenario));
    }

    #[test]
    fn a_different_seed_builds_a_different_world() {
        let one = World::build(&Scenario::behaving(1).partially_scanned(2));
        let other = World::build(&Scenario::behaving(2).partially_scanned(2));
        assert_ne!(one.libraries[0].items, other.libraries[0].items);
    }

    #[test]
    fn a_behaving_scenario_produces_no_misbehaviour_anywhere() {
        let world = World::build(&Scenario::behaving(1));
        for library in &world.libraries {
            for item in &library.items {
                assert!(item.indexed, "{}", item.rating_key);
                assert!(item.sort_title.is_some(), "{}", item.rating_key);
                assert!(!item.sort_title_locked, "{}", item.rating_key);
                assert!(
                    item.thumb.starts_with("/library/metadata/"),
                    "{}",
                    item.thumb
                );
            }
        }
    }

    #[test]
    fn asking_for_partial_scans_produces_some_and_not_all() {
        // All of them would be a different fake: the failure is a library where
        // some facts are unobservable and the rest look ordinary.
        let world = World::build(&Scenario::behaving(11).holding(60, 0).partially_scanned(3));
        let items = &world.libraries[0].items;
        assert!(items.iter().any(|item| !item.indexed));
        assert!(items.iter().any(|item| item.indexed));
    }

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
    fn a_scenario_that_changes_one_behaviour_leaves_the_others_where_they_were() {
        // The draws are unconditional so the stream's position does not depend
        // on which behaviours are on. Without that, turning on partial scans
        // would silently re-roll every sort title too.
        let base = World::build(&Scenario::behaving(3).holding(40, 0).absent_sort_titles(4));
        let with_scans = World::build(
            &Scenario::behaving(3)
                .holding(40, 0)
                .absent_sort_titles(4)
                .partially_scanned(3),
        );
        let sort_titles = |world: &World| {
            world.libraries[0]
                .items
                .iter()
                .map(|item| item.sort_title.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(sort_titles(&base), sort_titles(&with_scans));
    }

    #[test]
    fn churn_gives_every_item_a_new_key_and_keeps_its_identity() {
        let mut world = World::build(&Scenario::behaving(1));
        let before: Vec<(String, String)> = world.libraries[0]
            .items
            .iter()
            .map(|item| (item.rating_key.clone(), item.guid.clone()))
            .collect();
        world.churn_rating_keys();
        for (index, item) in world.libraries[0].items.iter().enumerate() {
            assert_ne!(item.rating_key, before[index].0);
            assert_eq!(item.guid, before[index].1);
        }
    }

    #[test]
    fn churn_does_not_empty_the_collections_the_items_are_in() {
        // The collection holds the item, not the number Plex prints for it. A
        // fake that dropped membership on every re-key would fail a client for
        // a reason no real server produces.
        let mut world = World::build(&Scenario::behaving(1));
        world.churn_rating_keys();
        let library = &world.libraries[0];
        let members = &library.collections[0].items;
        assert_eq!(members.len(), 3);
        for key in members {
            assert!(
                library.items.iter().any(|item| &item.rating_key == key),
                "{key} is no longer an item in the library"
            );
        }
    }

    #[test]
    fn every_library_has_an_anchor_the_plan_cannot_move_out_of_the_way() {
        let world = World::build(&Scenario::behaving(1));
        assert!(
            world.libraries[0]
                .hubs
                .iter()
                .any(|hub| hub.rating_key.is_none())
        );
    }
}
