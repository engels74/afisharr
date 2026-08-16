// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The whole world one run of the fake serves, and the ways it changes.

use crate::fake::{populate, scenario::Scenario, state::FakeLibrary};

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
    /// which is the property D-036 turns into an assertion. A scenario that
    /// declares more libraries is more draws from the same stream in the same
    /// order, never a different stream.
    pub(crate) fn build(scenario: &Scenario) -> Self {
        let mut seed = scenario.seed.rewound();
        let libraries = scenario
            .libraries
            .iter()
            .map(|spec| populate::library(scenario, &mut seed, spec))
            .collect();
        Self {
            machine_identifier: scenario.machine_identifier.clone(),
            version: scenario.version.clone(),
            friendly_name: scenario.friendly_name.clone(),
            libraries,
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

    /// The library holding this item.
    pub(crate) fn library_of_item(&mut self, item: &str) -> Option<&mut FakeLibrary> {
        self.libraries.iter_mut().find(|library| {
            library
                .items
                .iter()
                .any(|candidate| candidate.rating_key == item)
        })
    }

    /// Re-keys every item, keeping its guid.
    ///
    /// Rating-key churn (`I-ID-1`): the same logical item comes back under a
    /// new key, and a client that treated the key as identity now has two rows
    /// for one film. The guid is what survives, because it is the identity.
    pub(crate) fn churn_rating_keys(&mut self) {
        let every: Vec<String> = self
            .libraries
            .iter()
            .flat_map(|library| library.items.iter())
            .map(|item| item.rating_key.clone())
            .collect();
        for key in every {
            self.churn_one(&key);
        }
    }

    /// Re-keys one item, leaving every other item where it was.
    ///
    /// The case that breaks a cache. A wholesale churn is detectable by a
    /// caller comparing two whole windows; one item moving under a stable
    /// neighbourhood is what a pass reading a key it already holds walks into
    /// (`I-ID-1`, `I-SRC-6`).
    pub(crate) fn churn_one(&mut self, rating_key: &str) {
        let replacement = format!("{rating_key}9");
        for library in &mut self.libraries {
            for item in &mut library.items {
                if item.rating_key == rating_key {
                    item.rating_key.clone_from(&replacement);
                }
            }
            // Membership follows the item, because on a real server it does:
            // the collection holds the item, not the number Plex prints for it.
            // A fake that left the old key behind would drop the item from
            // every collection on every churn, and a test that then reported
            // the collection as lost would be reporting a failure no Plex
            // produces (PRD §21.10.2).
            for collection in &mut library.collections {
                for key in &mut collection.items {
                    if key == rating_key {
                        key.clone_from(&replacement);
                    }
                }
            }
        }
    }

    /// Gives one library a different section key, keeping its uuid.
    ///
    /// The same class of failure as a changed machine identifier, one level
    /// down: every stored `library.section_key` now addresses something else,
    /// and `uuid` is what PRD §19.7 matches on first so the library can still
    /// be recognised.
    pub(crate) fn rekey_section(&mut self, from: &str, to: &str) -> bool {
        let Some(library) = self.library(from) else {
            return false;
        };
        to.clone_into(&mut library.key);
        // The ordering space moves with the key, because on a real server it
        // does: Plex composes a hub identifier out of the section
        // (`custom.collection.{section}.{ratingKey}`,
        // `plexapi/collection.py:212`). Left alone, the manage answer would name
        // a section this server no longer has, and a test that re-keyed and then
        // read the space back would be reading rows for a world that is gone.
        //
        // Rating keys do not move. A section key changing is not an item
        // changing identity, and re-keying items here would fold the churn
        // misbehaviour into a call that is not it (`World::churn_one`).
        for hub in &mut library.hubs {
            hub.identifier = populate::rekey_identifier(&hub.identifier, from, to);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::scenario::{LibrarySpec, Scenario};

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
    fn the_default_world_is_the_two_libraries_every_earlier_test_was_written_against() {
        let world = World::build(&Scenario::behaving(1));
        assert_eq!(world.libraries.len(), 2);
        assert_eq!(world.libraries[0].key, "1");
        assert_eq!(world.libraries[0].items.len(), 12);
        assert_eq!(world.libraries[1].key, "2");
        assert_eq!(world.libraries[1].kind, "show");
    }

    #[test]
    fn a_scenario_can_declare_the_libraries_it_wants() {
        // Fixed at two, keyed 1 and 2, a second movie library and a music
        // library were both unreachable — and PRD §19.7's uuid-first matching
        // had nothing to match against.
        let world = World::build(&Scenario::behaving(1).with_libraries([
            LibrarySpec::of("7", "movie", "Films"),
            LibrarySpec::of("8", "movie", "Documentaries").holding(4),
            LibrarySpec::of("9", "artist", "Music"),
        ]));
        assert_eq!(world.libraries.len(), 3);
        assert_eq!(world.libraries[1].items.len(), 4);
        assert_eq!(world.libraries[2].kind, "artist");
        assert_ne!(world.libraries[0].uuid, world.libraries[1].uuid);
    }

    #[test]
    fn a_section_key_can_change_while_the_uuid_stays() {
        let mut world = World::build(&Scenario::behaving(1));
        let uuid = world.libraries[0].uuid.clone();
        assert!(world.rekey_section("1", "42"));
        assert_eq!(world.libraries[0].key, "42");
        assert_eq!(world.libraries[0].uuid, uuid);
        assert!(!world.rekey_section("1", "43"), "the old key is gone");
    }

    #[test]
    fn the_ordering_space_moves_with_the_section_key_and_the_rating_keys_do_not() {
        let mut world = World::build(&Scenario::behaving(1));
        let collection = world.libraries[0].collections[0].rating_key.clone();
        assert!(world.rekey_section("1", "42"));
        let hubs = &world.libraries[0].hubs;
        assert_eq!(hubs[0].identifier, "home.continue.42");
        assert_eq!(
            hubs[1].identifier,
            format!("custom.collection.42.{collection}")
        );
        assert_eq!(
            hubs[1].rating_key.as_deref(),
            Some(collection.as_str()),
            "a section changing key is not an item changing identity"
        );
        assert_eq!(world.libraries[0].collections[0].rating_key, collection);
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
    fn one_item_can_churn_while_every_other_key_stays() {
        // The case that breaks a cache: a wholesale re-key is detectable by
        // comparing two windows, and this one is not.
        let mut world = World::build(&Scenario::behaving(1));
        let moved = world.libraries[0].items[3].rating_key.clone();
        let untouched = world.libraries[0].items[4].rating_key.clone();
        world.churn_one(&moved);
        assert_eq!(world.libraries[0].items[3].rating_key, format!("{moved}9"));
        assert_eq!(world.libraries[0].items[4].rating_key, untouched);
    }

    #[test]
    fn churn_does_not_empty_the_collections_the_items_are_in() {
        // The collection holds the item, not the number Plex prints for it.
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
        assert!(world.libraries[0].hubs.iter().any(|hub| !hub.deletable));
    }
}
