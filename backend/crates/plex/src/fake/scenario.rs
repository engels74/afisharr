// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a run of the fake is asked to do.

use crate::fake::{
    plan::{FakeOperation, Injection, Trigger},
    seed::Seed,
};

/// The unrecognised artwork formats the fake serves (`I-ID-2`).
///
/// Two, because the invariant is about a *class* of failure: a client tested
/// against one unrecognised format can be written to special-case that one and
/// still fall over on the next. Neither is invented — both are shapes Plex has
/// used for internally stored artwork.
pub(crate) const UNRECOGNISED_ARTWORK: [&str; 2] = ["upload://posters/{key}", "blorp:?id={key}"];

/// One library a scenario asks the fake to serve.
///
/// Declared rather than fixed, because a world of exactly two libraries keyed
/// `1` and `2` put a section-key change, a second movie library, and a music
/// library all out of reach — and PRD §19.7's uuid-first matching had nothing
/// to match against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibrarySpec {
    pub(crate) key: String,
    pub(crate) kind: String,
    pub(crate) title: String,
    pub(crate) items: u32,
    pub(crate) smart_collection: bool,
}

impl LibrarySpec {
    /// A library under `key`, of `kind`, called `title`.
    #[must_use]
    pub fn of(key: impl Into<String>, kind: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            kind: kind.into(),
            title: title.into(),
            items: 12,
            smart_collection: false,
        }
    }

    /// How many items it holds.
    #[must_use]
    pub const fn holding(mut self, items: u32) -> Self {
        self.items = items;
        self
    }

    /// Makes this library's collection a smart one.
    ///
    /// The fake reports the flag and stops there; the refusals a smart
    /// collection produces live in the client that reads it
    /// (`plexapi/collection.py:317-318`, `:346-347`), and nothing could reach
    /// them while the fake had no way to mark one. A test that expects the
    /// *server* to refuse an item edit or an order change on one is expecting
    /// something this fake does not do.
    #[must_use]
    pub const fn with_smart_collection(mut self) -> Self {
        self.smart_collection = true;
        self
    }
}

/// A run of the fake, and everything it will do.
///
/// Built rather than configured field by field, so a scenario reads as the list
/// of misbehaviours a test is asking for, and a test that asks for none gets a
/// server that behaves.
#[derive(Debug, Clone)]
pub struct Scenario {
    pub(crate) seed: Seed,
    pub(crate) machine_identifier: String,
    pub(crate) version: String,
    pub(crate) friendly_name: String,
    pub(crate) libraries: Vec<LibrarySpec>,
    pub(crate) move_budget: u32,
    pub(crate) unrecognised_artwork_every: Option<u64>,
    pub(crate) partial_scan_every: Option<u64>,
    pub(crate) absent_sort_title_every: Option<u64>,
    pub(crate) locked_sort_title_every: Option<u64>,
    pub(crate) accepted_token: Option<String>,
    pub(crate) missing_item_answers_empty: bool,
    pub(crate) withholds_media_details: bool,
    pub(crate) injections: Vec<(FakeOperation, Trigger)>,
}

impl Scenario {
    /// A server that behaves, seeded from `seed`.
    ///
    /// Every misbehaviour is opt-in from here. A fake that misbehaved by
    /// default would make every test that did not ask for it a test of
    /// something nobody chose.
    #[must_use]
    pub fn behaving(seed: u64) -> Self {
        Self {
            seed: Seed::of(seed),
            machine_identifier: "fake-machine-0000".to_owned(),
            version: "1.41.0.0000-fake".to_owned(),
            friendly_name: "Fake Plex".to_owned(),
            libraries: vec![
                LibrarySpec::of("1", "movie", "Movies").holding(12),
                LibrarySpec::of("2", "show", "TV").holding(3),
            ],
            move_budget: u32::MAX,
            unrecognised_artwork_every: None,
            partial_scan_every: None,
            absent_sort_title_every: None,
            locked_sort_title_every: None,
            accepted_token: None,
            missing_item_answers_empty: false,
            withholds_media_details: false,
            injections: Vec::new(),
        }
    }

    /// The identifier this server answers with at the start.
    #[must_use]
    pub fn identified_as(mut self, machine_identifier: impl Into<String>) -> Self {
        self.machine_identifier = machine_identifier.into();
        self
    }

    /// The version this server reports.
    #[must_use]
    pub fn running_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// The libraries this server holds, replacing the default two.
    #[must_use]
    pub fn with_libraries(mut self, libraries: impl IntoIterator<Item = LibrarySpec>) -> Self {
        self.libraries = libraries.into_iter().collect();
        self
    }

    /// How many movies and shows the first library of each kind holds.
    ///
    /// Kept alongside [`Scenario::with_libraries`] because it is what almost
    /// every test asks for, and expressed in terms of the same declaration so
    /// there is one description of the world rather than two that can disagree.
    #[must_use]
    pub fn holding(mut self, movies: u32, shows: u32) -> Self {
        for (kind, count) in [("movie", movies), ("show", shows)] {
            if let Some(library) = self
                .libraries
                .iter_mut()
                .find(|library| library.kind == kind)
            {
                library.items = count;
            }
        }
        self
    }

    /// How many moves each sequence accepts before they silently no-op.
    ///
    /// The precision budget §15.3 describes, and one budget per sequence: the
    /// hub space has its own and so does every collection. Past it every move
    /// still answers 200 and changes nothing.
    #[must_use]
    pub const fn with_move_budget(mut self, moves: u32) -> Self {
        self.move_budget = moves;
        self
    }

    /// Serves an unrecognised artwork format for one item in `every`.
    #[must_use]
    pub const fn unrecognised_artwork(mut self, every: u64) -> Self {
        self.unrecognised_artwork_every = Some(every);
        self
    }

    /// Reports one item in `every` as still being indexed.
    #[must_use]
    pub const fn partially_scanned(mut self, every: u64) -> Self {
        self.partial_scan_every = Some(every);
        self
    }

    /// Omits the sort-title attribute on one item in `every`.
    ///
    /// Independent of the lock below, because §15.6 requires value, presence,
    /// and lock state to round-trip separately: an item with no sort title and
    /// a locked field is a real state, and it is the one a restore gets wrong.
    #[must_use]
    pub const fn absent_sort_titles(mut self, every: u64) -> Self {
        self.absent_sort_title_every = Some(every);
        self
    }

    /// Locks the sort-title field on one item in `every`.
    #[must_use]
    pub const fn locked_sort_titles(mut self, every: u64) -> Self {
        self.locked_sort_title_every = Some(every);
        self
    }

    /// Accepts only this token, refusing every other with `401`.
    ///
    /// A real server refuses (`plexapi/server.py:747-757`). Without this the
    /// revoked-credential state is provable only by an injected refusal, never
    /// by the condition the check exists to detect.
    #[must_use]
    pub fn accepting_token(mut self, token: impl Into<String>) -> Self {
        self.accepted_token = Some(token.into());
        self
    }

    /// Answers an empty container rather than `404` for an item it does not
    /// hold.
    ///
    /// Both shapes exist on real servers and a client has to survive each. The
    /// fake defaults to the refusal, because that is what a Plex answers to a
    /// rating key that has been re-keyed out from under a caller.
    #[must_use]
    pub const fn answering_empty_for_missing_items(mut self) -> Self {
        self.missing_item_answers_empty = true;
        self
    }

    /// Omits the media and stream attributes a server reports only sometimes.
    ///
    /// The absent-fact case: a client that read a missing `videoProfile` as
    /// "no profile" is reporting a fact nobody stated (P1).
    #[must_use]
    pub const fn withholding_media_details(mut self) -> Self {
        self.withholds_media_details = true;
        self
    }

    /// Makes `operation` misbehave, starting at its `after_calls`-th call.
    ///
    /// The call index is what makes it mid-pass: failing at call zero tests
    /// connection handling, and failing at call forty tests what happens to the
    /// work already done (`I-EVID-1`).
    #[must_use]
    pub fn failing(
        mut self,
        operation: FakeOperation,
        after_calls: u32,
        injection: Injection,
    ) -> Self {
        self.injections.push((
            operation,
            Trigger {
                after_calls,
                for_calls: None,
                injection,
            },
        ));
        self
    }

    /// Makes `operation` misbehave for a bounded run of calls, then recover.
    #[must_use]
    pub fn failing_for(
        mut self,
        operation: FakeOperation,
        after_calls: u32,
        for_calls: u32,
        injection: Injection,
    ) -> Self {
        self.injections.push((
            operation,
            Trigger {
                after_calls,
                for_calls: Some(for_calls),
                injection,
            },
        ));
        self
    }

    /// The same scenario, drawn from a different seed.
    ///
    /// For the assertion that makes the seed mean something: two runs of one
    /// scenario are identical, and two seeds are two worlds. Expressed as a
    /// change to an existing scenario so the comparison cannot accidentally be
    /// between two differently-built worlds.
    #[must_use]
    pub const fn reseeded(mut self, seed: u64) -> Self {
        self.seed = Seed::of(seed);
        self
    }

    /// The seed every behaviour in this scenario is drawn from.
    #[must_use]
    pub const fn seed(&self) -> &Seed {
        &self.seed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_behaving_scenario_asks_for_no_misbehaviour_at_all() {
        let scenario = Scenario::behaving(1);
        assert!(scenario.injections.is_empty());
        assert_eq!(scenario.unrecognised_artwork_every, None);
        assert_eq!(scenario.partial_scan_every, None);
        assert_eq!(scenario.move_budget, u32::MAX);
        assert_eq!(scenario.accepted_token, None);
        assert!(!scenario.missing_item_answers_empty);
    }

    #[test]
    fn every_misbehaviour_is_opted_into_by_name() {
        let scenario = Scenario::behaving(7)
            .identified_as("server-a")
            .holding(50, 5)
            .with_move_budget(3)
            .unrecognised_artwork(4)
            .partially_scanned(5)
            .absent_sort_titles(6)
            .locked_sort_titles(7)
            .accepting_token("the-only-one")
            .answering_empty_for_missing_items()
            .withholding_media_details()
            .failing(FakeOperation::Items, 2, Injection::Refuse { status: 503 });
        assert_eq!(scenario.machine_identifier, "server-a");
        assert_eq!(scenario.libraries[0].items, 50);
        assert_eq!(scenario.move_budget, 3);
        assert_eq!(scenario.unrecognised_artwork_every, Some(4));
        assert_eq!(scenario.partial_scan_every, Some(5));
        assert_eq!(scenario.absent_sort_title_every, Some(6));
        assert_eq!(scenario.locked_sort_title_every, Some(7));
        assert_eq!(scenario.accepted_token.as_deref(), Some("the-only-one"));
        assert!(scenario.missing_item_answers_empty);
        assert!(scenario.withholds_media_details);
        assert_eq!(scenario.injections.len(), 1);
    }

    #[test]
    fn the_item_counts_reach_the_declared_libraries_rather_than_a_second_field() {
        // Two descriptions of the world are two that can disagree, and the one
        // that loses is whichever the builder happens to read.
        let scenario = Scenario::behaving(1).holding(40, 6);
        assert_eq!(scenario.libraries[0].items, 40);
        assert_eq!(scenario.libraries[1].items, 6);
    }

    #[test]
    fn declared_libraries_replace_the_default_two_entirely() {
        let scenario = Scenario::behaving(1)
            .with_libraries([LibrarySpec::of("9", "artist", "Music").holding(2)]);
        assert_eq!(scenario.libraries.len(), 1);
        assert_eq!(scenario.libraries[0].kind, "artist");
    }

    #[test]
    fn a_bounded_failure_records_how_long_it_lasts() {
        let scenario =
            Scenario::behaving(1).failing_for(FakeOperation::Hubs, 1, 2, Injection::Stall);
        assert_eq!(scenario.injections[0].1.for_calls, Some(2));
    }

    #[test]
    fn the_scenario_carries_the_one_seed_everything_is_drawn_from() {
        assert_eq!(Scenario::behaving(99).seed().origin(), 99);
        assert_eq!(Scenario::behaving(99).reseeded(7).seed().origin(), 7);
    }

    #[test]
    fn the_unrecognised_formats_are_two_genuinely_different_shapes() {
        assert_eq!(UNRECOGNISED_ARTWORK.len(), 2);
        assert_ne!(UNRECOGNISED_ARTWORK[0], UNRECOGNISED_ARTWORK[1]);
    }
}
