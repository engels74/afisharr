// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One running fake: its world, and the misbehaviours it owes.

use std::{
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::fake::{
    library::World,
    plan::{FakeOperation, Injection, Injections},
    scenario::Scenario,
};

/// How long a stalled request is held.
///
/// Longer than any deadline this crate sets and short enough that a forgotten
/// task cannot outlive a test run. It is not a slow answer that eventually
/// arrives: nothing here answers, which is the failure a retry policy waiting
/// for an exception waits forever on.
const STALL: Duration = Duration::from_hours(1);

/// One running fake, as its handlers see it.
///
/// [`crate::fake::FakePlex`] is the handle a test holds; this is the state
/// behind it, and the two are separate because a test drives the fake from
/// outside while the handlers serve from inside.
///
/// A `Mutex` rather than an `RwLock`: almost every call here mutates — a move,
/// an edit, or the call counter the injection table advances — and a lock
/// chosen for read-mostly traffic would be the wrong shape for the traffic this
/// actually sees.
#[derive(Debug)]
pub(crate) struct FakeInstance {
    world: Mutex<World>,
    injections: Mutex<Injections>,
    churn_at_fetch: Mutex<Option<u32>>,
}

impl FakeInstance {
    /// Builds the running state a scenario describes.
    pub(crate) fn new(scenario: &Scenario) -> Self {
        let mut injections = Injections::default();
        for (operation, trigger) in &scenario.injections {
            injections.insert(*operation, *trigger);
        }
        Self {
            world: Mutex::new(World::build(scenario)),
            injections: Mutex::new(injections),
            churn_at_fetch: Mutex::new(None),
        }
    }

    /// The world, for a handler or for a test's assertions.
    pub(crate) fn world(&self) -> MutexGuard<'_, World> {
        self.world.lock().unwrap_or_else(|poisoned| {
            // A handler panicked while holding it. The fake's own state is the
            // only thing at risk and a test is already failing; recovering
            // keeps the failure the test's rather than a second panic here.
            poisoned.into_inner()
        })
    }

    /// Asks for rating-key churn on the `after`-th item-list fetch.
    pub(crate) fn churn_at_fetch(&self, after: Option<u32>) {
        *self
            .churn_at_fetch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = after;
    }

    /// Counts one item-list fetch, churning the keys if this is the one.
    pub(crate) fn note_fetch(&self) {
        let scheduled = *self
            .churn_at_fetch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut world = self.world();
        let fetch = world.fetches;
        world.fetches = fetch.saturating_add(1);
        if scheduled == Some(fetch) {
            world.churn_rating_keys();
        }
    }

    /// Counts one call to `operation`, and misbehaves if the scenario said to.
    ///
    /// Every handler calls this first. Answering the request and *then*
    /// checking would make a 5xx injection a failure that still wrote — which
    /// is not a failure any real server produces, and would let a test pass
    /// against a client that ignores the status.
    pub(crate) async fn gate(&self, operation: FakeOperation) -> Option<Response> {
        let injection = self
            .injections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .advance(operation)?;
        match injection {
            Injection::Refuse { status } => Some(
                (
                    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    "the fake was asked to refuse this call",
                )
                    .into_response(),
            ),
            Injection::Stall => {
                tokio::time::sleep(STALL).await;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::plan::Injection;

    #[tokio::test]
    async fn a_behaving_scenario_gates_nothing() {
        let running = FakeInstance::new(&Scenario::behaving(1));
        assert!(running.gate(FakeOperation::Items).await.is_none());
    }

    #[tokio::test]
    async fn a_refusal_answers_the_status_the_scenario_named() {
        let running = FakeInstance::new(&Scenario::behaving(1).failing(
            FakeOperation::Items,
            0,
            Injection::Refuse { status: 503 },
        ));
        let response = running
            .gate(FakeOperation::Items)
            .await
            .expect("the first call refuses");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn churn_lands_on_the_fetch_it_was_asked_for() {
        let running = FakeInstance::new(&Scenario::behaving(1));
        let before = running.world().libraries[0].items[0].rating_key.clone();
        running.churn_at_fetch(Some(1));

        running.note_fetch();
        assert_eq!(running.world().libraries[0].items[0].rating_key, before);

        running.note_fetch();
        assert_ne!(running.world().libraries[0].items[0].rating_key, before);
    }

    #[test]
    fn no_churn_is_scheduled_by_default() {
        let running = FakeInstance::new(&Scenario::behaving(1));
        let before = running.world().libraries[0].items[0].rating_key.clone();
        for _ in 0..5 {
            running.note_fetch();
        }
        assert_eq!(running.world().libraries[0].items[0].rating_key, before);
    }
}
