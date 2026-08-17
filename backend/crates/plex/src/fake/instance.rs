// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One running fake: its world, what it refuses, and the misbehaviours it owes.

use std::{
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use axum::{http::StatusCode, response::Response};

use crate::fake::{
    library::World,
    negotiation::Rendering,
    plan::{FakeOperation, Injection, Injections},
    request::Arguments,
    scenario::Scenario,
    shape::Detail,
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
    accepted_token: Option<String>,
    missing_item_answers_empty: bool,
    withholds_media_details: bool,
    move_budget: u32,
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
            accepted_token: scenario.accepted_token.clone(),
            missing_item_answers_empty: scenario.missing_item_answers_empty,
            withholds_media_details: scenario.withholds_media_details,
            move_budget: scenario.move_budget,
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

    /// Whether this server accepts the token a request presented.
    ///
    /// A scenario that names one accepts only that. A scenario that names none
    /// accepts any token at all and refuses a request carrying none — which is
    /// what a claimed server does, and what makes `verify_credential` provable
    /// by the condition rather than only by an injected refusal.
    pub(crate) fn accepts_token(&self, presented: Option<&str>) -> bool {
        match (&self.accepted_token, presented) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(accepted), Some(presented)) => accepted == presented,
        }
    }

    /// Whether a missing item is an empty container rather than a `404`.
    pub(crate) const fn missing_item_answers_empty(&self) -> bool {
        self.missing_item_answers_empty
    }

    /// The move budget every sequence this world builds starts with.
    pub(crate) const fn move_budget(&self) -> u32 {
        self.move_budget
    }

    /// What one request is told about media, given what it asked for.
    pub(crate) fn detail(&self, arguments: &Arguments) -> Detail {
        Detail {
            check_files: arguments.flag("checkFiles"),
            include_guids: arguments.flag("includeGuids"),
            withhold: self.withholds_media_details,
        }
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
    pub(crate) async fn gate(
        &self,
        operation: FakeOperation,
        rendering: Rendering,
    ) -> Option<Response> {
        let injection = self
            .injections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .advance(operation)?;
        match injection {
            // In the envelope, and in the rendering the request asked for: a
            // real Plex refuses inside the same envelope as everything else,
            // and a fake whose injected refusals answered bare text would make
            // a client that parses a refusal body fail here for a reason no
            // server produces (see [`Rendering::refusal`]).
            Injection::Refuse { status } => Some(rendering.refusal(
                StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                status,
                "the fake was asked to refuse this call",
            )),
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
        assert!(
            running
                .gate(FakeOperation::Items, Rendering::Json)
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn a_refusal_answers_the_status_the_scenario_named() {
        let running = FakeInstance::new(&Scenario::behaving(1).failing(
            FakeOperation::Items,
            0,
            Injection::Refuse { status: 503 },
        ));
        let response = running
            .gate(FakeOperation::Items, Rendering::Json)
            .await
            .expect("the first call refuses");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn a_server_that_names_no_token_still_refuses_a_request_carrying_none() {
        let running = FakeInstance::new(&Scenario::behaving(1));
        assert!(running.accepts_token(Some("anything")));
        assert!(!running.accepts_token(None));
    }

    #[test]
    fn a_server_that_names_a_token_accepts_only_that_one() {
        let running = FakeInstance::new(&Scenario::behaving(1).accepting_token("the-only-one"));
        assert!(running.accepts_token(Some("the-only-one")));
        assert!(!running.accepts_token(Some("a-revoked-one")));
        assert!(!running.accepts_token(None));
    }

    #[test]
    fn a_file_check_is_reported_only_when_the_request_asked_for_one() {
        let running = FakeInstance::new(&Scenario::behaving(1));
        assert!(!running.detail(&Arguments::default()).check_files);
        assert!(
            running
                .detail(&Arguments::parse(Some("checkFiles=1")))
                .check_files
        );
        assert!(!running.detail(&Arguments::default()).withhold);
        assert!(
            FakeInstance::new(&Scenario::behaving(1).withholding_media_details())
                .detail(&Arguments::default())
                .withhold
        );
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
