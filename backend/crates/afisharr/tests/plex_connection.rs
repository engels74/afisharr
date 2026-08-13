// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Task 2.4 and `I-ID-5`, end to end against the adversarial fake.
//!
//! The invariant's test is stated as: change the machine identifier between
//! passes, assert zero writes and a blocking finding. Phase 2 has no passes
//! yet, so the pass here is the connectivity check — the first thing anything
//! Plex-bound does, and deliberately the cheapest. What it must never do is
//! rebind, and what it must always do is name both identifiers, because the
//! decision it hands back to the operator needs both.

mod harness;

use afisharr_core::{
    plex_server::PlexServer,
    secrets::{PutSecret, SecretKey},
    storage::WriteOperation,
    time::Timestamp,
};
use afisharr_plex::fake::{FakePlex, Scenario};
use harness::{RunningInstance, TempInstance, Wizard, browser, csrf_from};
use reqwest::{Client, StatusCode};

const PASSWORD: &str = "correct horse battery staple";

/// A configured instance with an administrator signed in.
async fn signed_in(instance: &TempInstance) -> (RunningInstance, Client, String) {
    let running = RunningInstance::start(instance).await;
    let _wizard = Wizard::set_up(&running, "operator", PASSWORD).await;
    let client = browser();
    let signed_in = client
        .post(format!("{}/api/auth/login", running.base_url))
        .json(&serde_json::json!({ "username": "operator", "password": PASSWORD }))
        .send()
        .await
        .expect("the login route must answer");
    assert_eq!(signed_in.status(), StatusCode::OK);
    let csrf = csrf_from(&signed_in).expect("signing in must set the CSRF cookie");
    (running, client, csrf)
}

/// Runs the check and returns the body it answered with.
async fn check(running: &RunningInstance, client: &Client, csrf: &str) -> serde_json::Value {
    let response = client
        .post(format!(
            "{}/api/settings/plex/connection/check",
            running.base_url
        ))
        .header("x-afisharr-csrf", csrf)
        .send()
        .await
        .expect("the check route must answer");
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "the check itself succeeds"
    );
    response.json().await.expect("the answer is JSON")
}

/// Binds the instance to `fake`, as the wizard's Plex step will.
async fn bind(running: &RunningInstance, fake: &FakePlex) {
    running
        .booted
        .database
        .writer()
        .submit(BindServer {
            machine_identifier: fake.machine_identifier(),
            base_url: fake.base_url().to_owned(),
        })
        .await
        .expect("the binding must be writable");
}

struct BindServer {
    machine_identifier: String,
    base_url: String,
}

impl WriteOperation for BindServer {
    type Output = ();

    async fn execute(self, conn: &mut sqlx::SqliteConnection) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO plex_server
                 (id, machine_identifier, friendly_name, version, base_url,
                  first_seen_at, last_seen_at)
             VALUES (1, ?1, 'Fake Plex', '1.0.0-before', ?2, 1000, 1000)",
        )
        .bind(self.machine_identifier)
        .bind(self.base_url)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

/// Stores a Plex server token, as a completed sign-in does.
async fn store_token(running: &RunningInstance, key: &SecretKey) {
    running
        .booted
        .database
        .writer()
        .submit(PutSecret {
            name: "plex.token".to_owned(),
            sealed: key
                .seal(b"a-plex-server-token")
                .expect("the token must seal"),
            at: Timestamp::from_millis(1_000),
        })
        .await
        .expect("the secret must be writable");
}

/// The bound server exactly as the database holds it.
async fn recorded(running: &RunningInstance) -> Option<PlexServer> {
    afisharr_core::plex_server::load(running.booted.database.readers())
        .await
        .expect("the binding must be readable")
}

#[tokio::test]
async fn an_installation_with_no_server_says_so_rather_than_reporting_a_failure() {
    // "Nothing is configured" and "the server did not answer" are opposite
    // problems, and an operator shown the second for the first goes looking for
    // a network fault that is not there (P1).
    let instance = TempInstance::new();
    let (running, client, csrf) = signed_in(&instance).await;

    let answer = check(&running, &client, &csrf).await;
    assert_eq!(answer["state"], "notConfigured");
    assert!(answer["boundMachineIdentifier"].is_null());
    assert!(answer["observedMachineIdentifier"].is_null());

    running.stop().await;
}

#[tokio::test]
async fn a_bound_server_with_no_stored_token_is_its_own_state() {
    let fake = FakePlex::start(Scenario::behaving(1).identified_as("server-a")).await;
    let instance = TempInstance::new();
    let (running, client, csrf) = signed_in(&instance).await;
    bind(&running, &fake).await;

    let answer = check(&running, &client, &csrf).await;
    assert_eq!(answer["state"], "noCredential");
    assert_eq!(answer["boundMachineIdentifier"], "server-a");
    // Nothing was observed, because nothing was asked: reporting an identifier
    // here would be reporting an observation that never happened.
    assert!(answer["observedMachineIdentifier"].is_null());

    running.stop().await;
}

#[tokio::test]
async fn the_bound_server_answering_is_reachable_and_refreshes_what_it_reported() {
    let fake = FakePlex::start(
        Scenario::behaving(1)
            .identified_as("server-a")
            .running_version("1.41.9.9999-fake"),
    )
    .await;
    let instance = TempInstance::new();
    let (running, client, csrf) = signed_in(&instance).await;
    bind(&running, &fake).await;
    store_token(&running, &running.booted.secret_key).await;

    let answer = check(&running, &client, &csrf).await;
    assert_eq!(answer["state"], "reachable");
    assert_eq!(answer["boundMachineIdentifier"], "server-a");
    assert_eq!(answer["observedMachineIdentifier"], "server-a");
    assert_eq!(answer["version"], "1.41.9.9999-fake");

    // The observation is recorded: the version drives discovered-field
    // invalidation (PRD §19.8), and a check that read it and threw it away
    // would leave the cache keyed on a version the server no longer runs.
    let row = recorded(&running).await.expect("the binding survives");
    assert_eq!(row.version, "1.41.9.9999-fake");
    assert!(
        row.last_version_change_at.is_some(),
        "the version changed, and the change is when it happened"
    );

    running.stop().await;
}

#[tokio::test]
async fn a_server_that_does_not_answer_is_unreachable_and_nothing_is_rebound() {
    let fake = FakePlex::start(Scenario::behaving(1).identified_as("server-a")).await;
    let base_url = fake.base_url().to_owned();
    let instance = TempInstance::new();
    let (running, client, csrf) = signed_in(&instance).await;
    bind(&running, &fake).await;
    store_token(&running, &running.booted.secret_key).await;
    // The server goes away, and the address stays configured. That is what an
    // operator's restarted container looks like from here.
    fake.stop();

    let answer = check(&running, &client, &csrf).await;
    assert_eq!(answer["state"], "unreachable");
    assert_eq!(answer["boundMachineIdentifier"], "server-a");
    assert!(
        answer["observedMachineIdentifier"].is_null(),
        "nothing answered, so nothing was observed"
    );
    assert!(
        answer["detail"]
            .as_str()
            .is_some_and(|detail| !detail.is_empty()),
        "the collapsed technical detail is kept for the operator (§8.4)"
    );

    let row = recorded(&running).await.expect("the binding survives");
    assert_eq!(row.machine_identifier, "server-a");
    assert_eq!(row.base_url, base_url);

    running.stop().await;
}

#[tokio::test]
async fn a_different_server_blocks_writes_nothing_and_names_both_identifiers() {
    // `I-ID-5`. The operator pointed the same address at another machine.
    let fake = FakePlex::start(Scenario::behaving(1).identified_as("server-a")).await;
    let instance = TempInstance::new();
    let (running, client, csrf) = signed_in(&instance).await;
    bind(&running, &fake).await;
    store_token(&running, &running.booted.secret_key).await;

    let bound = check(&running, &client, &csrf).await;
    assert_eq!(bound["state"], "reachable");
    let before = recorded(&running).await.expect("the binding exists");

    fake.becomes_a_different_server("server-b");

    let answer = check(&running, &client, &csrf).await;
    assert_eq!(answer["state"], "wrongServer");
    assert_eq!(
        answer["boundMachineIdentifier"], "server-a",
        "what the operator is being asked to abandon"
    );
    assert_eq!(
        answer["observedMachineIdentifier"], "server-b",
        "what answered instead"
    );

    // Zero writes. Not the identifier, not the version, not even `last_seen_at`
    // — the row describes the server this installation is bound to, and
    // touching any of it on a stranger's answer is the beginning of the silent
    // rebind the invariant forbids.
    let after = recorded(&running).await.expect("the binding survives");
    assert_eq!(
        after, before,
        "a different server answering must change no row"
    );

    running.stop().await;
}

#[tokio::test]
async fn the_block_persists_until_something_changes_and_never_resolves_itself() {
    // "Neither auto-resolved": running the check again is not a decision, and
    // an instance that rebound on the second look would rebind on every one.
    let fake = FakePlex::start(Scenario::behaving(1).identified_as("server-a")).await;
    let instance = TempInstance::new();
    let (running, client, csrf) = signed_in(&instance).await;
    bind(&running, &fake).await;
    store_token(&running, &running.booted.secret_key).await;
    fake.becomes_a_different_server("server-b");

    for attempt in 0..3 {
        let answer = check(&running, &client, &csrf).await;
        assert_eq!(answer["state"], "wrongServer", "attempt {attempt}");
    }
    let row = recorded(&running).await.expect("the binding survives");
    assert_eq!(row.machine_identifier, "server-a");

    // And putting the original server back clears it, with no operator action:
    // the binding was never wrong, the address was pointed elsewhere.
    fake.becomes_a_different_server("server-a");
    let answer = check(&running, &client, &csrf).await;
    assert_eq!(answer["state"], "reachable");

    running.stop().await;
}

#[tokio::test]
async fn the_check_is_administrator_only_and_scoped() {
    let instance = TempInstance::new();
    let (running, client, csrf) = signed_in(&instance).await;

    // A key issued to browse the filesystem has no business making this
    // instance talk to Plex.
    let issued: serde_json::Value = client
        .post(format!("{}/api/settings/api-keys", running.base_url))
        .header("x-afisharr-csrf", &csrf)
        .json(&serde_json::json!({ "name": "A viewer", "scopes": ["files:read"] }))
        .send()
        .await
        .expect("the key route must answer")
        .json()
        .await
        .expect("the answer is JSON");
    let narrow = issued["secret"].as_str().expect("a plaintext key");

    let refused = browser()
        .post(format!(
            "{}/api/settings/plex/connection/check",
            running.base_url
        ))
        .bearer_auth(narrow)
        .send()
        .await
        .expect("the check route must answer");
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    let problem: serde_json::Value = refused.json().await.expect("the answer is JSON");
    assert!(
        problem["message"]
            .as_str()
            .is_some_and(|message| message.contains("plex:read")),
        "the scope to re-issue with must be named: {problem}"
    );

    running.stop().await;
}
