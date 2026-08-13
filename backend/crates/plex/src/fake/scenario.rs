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
    pub(crate) movies: u32,
    pub(crate) shows: u32,
    pub(crate) move_budget: u32,
    pub(crate) unrecognised_artwork_every: Option<u64>,
    pub(crate) partial_scan_every: Option<u64>,
    pub(crate) absent_sort_title_every: Option<u64>,
    pub(crate) locked_sort_title_every: Option<u64>,
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
            movies: 12,
            shows: 3,
            move_budget: u32::MAX,
            unrecognised_artwork_every: None,
            partial_scan_every: None,
            absent_sort_title_every: None,
            locked_sort_title_every: None,
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

    /// How many movies and shows the library holds.
    #[must_use]
    pub const fn holding(mut self, movies: u32, shows: u32) -> Self {
        self.movies = movies;
        self.shows = shows;
        self
    }

    /// How many moves the ordering space accepts before they silently no-op.
    ///
    /// The precision budget §15.3 describes. Past it every move still answers
    /// 200 and changes nothing.
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
            .failing(FakeOperation::Items, 2, Injection::Refuse { status: 503 });
        assert_eq!(scenario.machine_identifier, "server-a");
        assert_eq!(scenario.movies, 50);
        assert_eq!(scenario.move_budget, 3);
        assert_eq!(scenario.unrecognised_artwork_every, Some(4));
        assert_eq!(scenario.partial_scan_every, Some(5));
        assert_eq!(scenario.absent_sort_title_every, Some(6));
        assert_eq!(scenario.locked_sort_title_every, Some(7));
        assert_eq!(scenario.injections.len(), 1);
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
    }

    #[test]
    fn the_unrecognised_formats_are_two_genuinely_different_shapes() {
        // One would let a client special-case it and still fall over on the
        // next, which is the opposite of what `I-ID-2` is testing.
        assert_eq!(UNRECOGNISED_ARTWORK.len(), 2);
        assert_ne!(UNRECOGNISED_ARTWORK[0], UNRECOGNISED_ARTWORK[1]);
    }
}
