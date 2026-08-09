// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Projecting `library_item_state.state_hash`.

use crate::digest;

/// The three inputs `state_hash` is a digest over (PRD §19.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateInputs<'a> {
    /// Digest of the resolved `media.*` / `item.*` values.
    pub facts_hash: &'a str,
    /// Digest of the ratings, when they have ever been fetched.
    ///
    /// `None` means *unavailable* — the ratings have never been fetched, or the
    /// last fetch failed. That is a different fact from a fetch that returned
    /// no ratings, and the schema keeps the two apart deliberately, so the
    /// projection keeps them apart too (P1).
    pub ratings_hash: Option<&'a str>,
    /// The subject's four lifecycle axes, when a subject exists for this item.
    pub lifecycle: Option<LifecycleAxes<'a>>,
}

/// The lifecycle state of the subject bound to an item.
///
/// The axes are hashed rather than the derived status of PRD §17.7, because the
/// derived status is a function of the axes: hashing the inputs cannot miss a
/// change that the function would have surfaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecycleAxes<'a> {
    /// Where the title sits on its own calendar.
    pub phase: &'a str,
    /// What the download stack is doing.
    pub acquisition: &'a str,
    /// What Afisharr has put in the library.
    pub presence: &'a str,
    /// Production state; TV only, so `None` for a film.
    pub production: Option<&'a str>,
}

/// The sentinel written where an input is unobservable rather than empty.
///
/// A literal that cannot collide with a hex digest or a state token, so
/// "ratings have never been fetched" and "ratings hashed to the empty string"
/// are different digests.
const ABSENT: &str = "\u{1}absent";

/// Computes `state_hash` from the inputs the row and its subject carry.
///
/// The render key includes this, so it decides when an overlay is re-rendered.
/// Under-invalidating ships a stale poster; the inputs are therefore joined with
/// a separator that cannot appear inside any of them.
#[must_use]
pub fn project_state_hash(inputs: &StateInputs<'_>) -> String {
    let lifecycle = inputs.lifecycle.map_or_else(
        || ABSENT.to_owned(),
        |axes| {
            format!(
                "{}\u{0}{}\u{0}{}\u{0}{}",
                axes.phase,
                axes.acquisition,
                axes.presence,
                axes.production.unwrap_or(ABSENT)
            )
        },
    );

    digest::hex(format!(
        "{}\u{0}{}\u{0}{lifecycle}",
        inputs.facts_hash,
        inputs.ratings_hash.unwrap_or(ABSENT)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axes() -> LifecycleAxes<'static> {
        LifecycleAxes {
            phase: "Released",
            acquisition: "Available",
            presence: "Real",
            production: None,
        }
    }

    fn inputs() -> StateInputs<'static> {
        StateInputs {
            facts_hash: "f00d",
            ratings_hash: Some("bead"),
            lifecycle: Some(axes()),
        }
    }

    #[test]
    fn the_hash_is_stable_for_identical_inputs() {
        assert_eq!(project_state_hash(&inputs()), project_state_hash(&inputs()));
    }

    #[test]
    fn changing_the_facts_changes_the_hash() {
        let mut changed = inputs();
        changed.facts_hash = "f00e";
        assert_ne!(project_state_hash(&inputs()), project_state_hash(&changed));
    }

    #[test]
    fn changing_the_ratings_changes_the_hash() {
        let mut changed = inputs();
        changed.ratings_hash = Some("beae");
        assert_ne!(project_state_hash(&inputs()), project_state_hash(&changed));
    }

    #[test]
    fn changing_any_lifecycle_axis_changes_the_hash() {
        let baseline = project_state_hash(&inputs());
        for mutate in [
            |a: &mut LifecycleAxes<'static>| a.phase = "Countdown",
            |a: &mut LifecycleAxes<'static>| a.acquisition = "Grabbing",
            |a: &mut LifecycleAxes<'static>| a.presence = "Placeholder",
            |a: &mut LifecycleAxes<'static>| a.production = Some("Airing"),
        ] {
            let mut changed_axes = axes();
            mutate(&mut changed_axes);
            let mut changed = inputs();
            changed.lifecycle = Some(changed_axes);
            assert_ne!(baseline, project_state_hash(&changed));
        }
    }

    #[test]
    fn unavailable_ratings_differ_from_ratings_that_hashed_to_nothing() {
        let mut unavailable = inputs();
        unavailable.ratings_hash = None;
        let mut empty = inputs();
        empty.ratings_hash = Some("");
        assert_ne!(project_state_hash(&unavailable), project_state_hash(&empty));
    }

    #[test]
    fn an_item_with_no_subject_differs_from_one_with_a_subject() {
        let mut unbound = inputs();
        unbound.lifecycle = None;
        assert_ne!(project_state_hash(&inputs()), project_state_hash(&unbound));
    }

    #[test]
    fn the_separator_stops_field_boundaries_from_sliding() {
        let shifted = StateInputs {
            facts_hash: "f00",
            ratings_hash: Some("dbead"),
            lifecycle: Some(axes()),
        };
        assert_ne!(project_state_hash(&inputs()), project_state_hash(&shifted));
    }
}
