// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Which operation misbehaves, and how.

use std::collections::HashMap;

/// One call the fake serves.
///
/// A closed enum rather than a path string, so a test that asks for a failure
/// "at the move call" cannot ask for it at a path that no longer exists: a
/// renamed endpoint is a compile error here, and a silently unmatched string
/// would be a scenario that quietly injects nothing and a test that quietly
/// passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FakeOperation {
    /// `GET /` — the call a real server answers only to an accepted token.
    ///
    /// Its own operation rather than a variant of [`Self::Identity`], because
    /// the two differ in exactly the way a test needs: refusing this one and
    /// answering that one is what a revoked token looks like from outside, and
    /// it is the state a check built on the identity call alone cannot see.
    Root,
    /// `GET /identity`.
    Identity,
    /// `GET /library/sections`.
    Sections,
    /// `GET /library/sections/{key}/all`.
    Items,
    /// `GET /library/metadata/{key}`.
    Item,
    /// `GET /library/sections/{key}/collections`.
    Collections,
    /// `POST /library/collections`.
    CreateCollection,
    /// `PUT /library/sections/{key}/all` for a collection.
    EditCollection,
    /// `DELETE /library/collections/{key}`.
    DeleteCollection,
    /// `GET /library/collections/{key}/children`.
    CollectionItems,
    /// `PUT /library/collections/{key}/items`.
    AddCollectionItems,
    /// `DELETE /library/collections/{key}/items/{item}`.
    RemoveCollectionItem,
    /// `PUT /library/collections/{key}/items/{item}/move`.
    MoveCollectionItem,
    /// `GET /hubs/sections/{key}/manage`.
    Hubs,
    /// `PUT /hubs/sections/{key}/manage/{hub}/move`.
    MoveHub,
    /// `PUT /hubs/sections/{key}/manage/{hub}`.
    SetHubVisibility,
    /// `PUT /library/sections/{key}/all` for labels.
    EditLabels,
    /// `POST /library/metadata/{key}/posters`.
    UploadPoster,
    /// `GET /library/sections/{key}/all?includeMeta=1`.
    Vocabulary,
    /// `GET` a filter's choice list.
    FilterChoices,
}

/// How an operation misbehaves when it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Injection {
    /// Answer with a server error.
    ///
    /// The status is named rather than assumed, because 500, 502, and 503 mean
    /// different things to a retry policy and a test that only ever saw one of
    /// them proves nothing about the other two.
    Refuse {
        /// The status to answer with.
        status: u16,
    },
    /// Accept the request and never answer it.
    ///
    /// Not a slow answer: a stalled connection is the failure the deadline in
    /// PRD §21.2.5 exists for, and it is the one a retry policy waiting for an
    /// exception waits forever on.
    Stall,
}

/// When an operation misbehaves, counted from the first call to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Trigger {
    /// How many calls answer normally first. `0` fails the very first one.
    pub(crate) after_calls: u32,
    /// How many consecutive calls misbehave once it starts. `None` is "every
    /// call from here on".
    pub(crate) for_calls: Option<u32>,
    /// What happens while it is misbehaving.
    pub(crate) injection: Injection,
}

/// The failures a scenario injects, and how many calls each has seen.
///
/// Mid-pass injection is the whole point of the counter: a pass that fails on
/// its first call tests connection handling, and a pass that fails on its
/// fortieth tests what happens to work already done (`I-EVID-1`).
#[derive(Debug, Default)]
pub(crate) struct Injections {
    triggers: HashMap<FakeOperation, Vec<Trigger>>,
    seen: HashMap<FakeOperation, u32>,
}

impl Injections {
    /// Records that `operation` misbehaves.
    ///
    /// Appended rather than replacing what is already there. A scenario names
    /// its failures one at a time — "refuse the first call, then stall calls
    /// three to five" is two of them on one operation — and a table that kept
    /// only the last would drop the first silently, leaving a test asserting
    /// against a scenario nobody wrote.
    pub(crate) fn insert(&mut self, operation: FakeOperation, trigger: Trigger) {
        self.triggers.entry(operation).or_default().push(trigger);
    }

    /// Counts one call to `operation`, and says what it should do.
    ///
    /// The first trigger whose window covers this call wins, in the order the
    /// scenario named them.
    pub(crate) fn advance(&mut self, operation: FakeOperation) -> Option<Injection> {
        let seen = self.seen.entry(operation).or_default();
        let index = *seen;
        *seen = seen.saturating_add(1);

        self.triggers
            .get(&operation)?
            .iter()
            .find(|trigger| trigger.covers(index))
            .map(|trigger| trigger.injection)
    }
}

impl Trigger {
    /// Whether this trigger is misbehaving on the call at `index`.
    const fn covers(&self, index: u32) -> bool {
        if index < self.after_calls {
            return false;
        }
        match self.for_calls {
            Some(count) => index < self.after_calls.saturating_add(count),
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn injections(trigger: Trigger) -> Injections {
        let mut injections = Injections::default();
        injections.insert(FakeOperation::Items, trigger);
        injections
    }

    #[test]
    fn an_operation_with_no_trigger_always_answers_normally() {
        let mut injections = Injections::default();
        for _ in 0..10 {
            assert_eq!(injections.advance(FakeOperation::Items), None);
        }
    }

    #[test]
    fn a_failure_lands_on_the_call_it_was_asked_for_and_not_before() {
        let mut injections = injections(Trigger {
            after_calls: 3,
            for_calls: None,
            injection: Injection::Refuse { status: 503 },
        });
        for call in 0..3 {
            assert_eq!(
                injections.advance(FakeOperation::Items),
                None,
                "call {call}"
            );
        }
        assert_eq!(
            injections.advance(FakeOperation::Items),
            Some(Injection::Refuse { status: 503 })
        );
    }

    #[test]
    fn a_bounded_failure_stops_and_the_operation_recovers() {
        // The shape a retry test needs: fail twice, then work. A permanent
        // failure cannot tell a retry that succeeded from one that gave up.
        let mut injections = injections(Trigger {
            after_calls: 1,
            for_calls: Some(2),
            injection: Injection::Stall,
        });
        assert_eq!(injections.advance(FakeOperation::Items), None);
        assert_eq!(
            injections.advance(FakeOperation::Items),
            Some(Injection::Stall)
        );
        assert_eq!(
            injections.advance(FakeOperation::Items),
            Some(Injection::Stall)
        );
        assert_eq!(injections.advance(FakeOperation::Items), None);
    }

    #[test]
    fn two_failures_on_one_operation_both_survive() {
        // A scenario names its failures one at a time, and a table that kept
        // only the last would drop the first without saying so.
        let mut injections = injections(Trigger {
            after_calls: 0,
            for_calls: Some(1),
            injection: Injection::Refuse { status: 503 },
        });
        injections.insert(
            FakeOperation::Items,
            Trigger {
                after_calls: 2,
                for_calls: None,
                injection: Injection::Stall,
            },
        );
        assert_eq!(
            injections.advance(FakeOperation::Items),
            Some(Injection::Refuse { status: 503 })
        );
        assert_eq!(injections.advance(FakeOperation::Items), None);
        assert_eq!(
            injections.advance(FakeOperation::Items),
            Some(Injection::Stall)
        );
    }

    #[test]
    fn each_operation_counts_its_own_calls() {
        // A pass calls several endpoints. Counting them together would put the
        // failure wherever the traffic happened to fall, which is the opposite
        // of a scenario.
        let mut injections = injections(Trigger {
            after_calls: 1,
            for_calls: None,
            injection: Injection::Refuse { status: 500 },
        });
        assert_eq!(injections.advance(FakeOperation::Hubs), None);
        assert_eq!(injections.advance(FakeOperation::Hubs), None);
        assert_eq!(injections.advance(FakeOperation::Items), None);
        assert_eq!(
            injections.advance(FakeOperation::Items),
            Some(Injection::Refuse { status: 500 })
        );
    }
}
