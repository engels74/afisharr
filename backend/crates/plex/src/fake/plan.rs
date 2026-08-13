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
    triggers: HashMap<FakeOperation, Trigger>,
    seen: HashMap<FakeOperation, u32>,
}

impl Injections {
    /// Records that `operation` misbehaves.
    pub(crate) fn insert(&mut self, operation: FakeOperation, trigger: Trigger) {
        self.triggers.insert(operation, trigger);
    }

    /// Counts one call to `operation`, and says what it should do.
    pub(crate) fn advance(&mut self, operation: FakeOperation) -> Option<Injection> {
        let seen = self.seen.entry(operation).or_default();
        let index = *seen;
        *seen = seen.saturating_add(1);

        let trigger = self.triggers.get(&operation)?;
        if index < trigger.after_calls {
            return None;
        }
        match trigger.for_calls {
            Some(count) if index >= trigger.after_calls.saturating_add(count) => None,
            _ => Some(trigger.injection),
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
