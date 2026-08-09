// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Task 1.2: the plex.tv PIN and OAuth flows, driven end to end.
//!
//! Against a stand-in for plex.tv rather than the real service. The
//! adversarial fake (D-036) arrives in Phase 2 and every phase from Phase 4
//! onward tests against it; this is the smallest thing that makes the exchange
//! provable now.

mod harness;

use afisharr_core::{accounts, storage::WriteOperation};
use harness::{PlexTvStub, RunningInstance, TempInstance, Wizard};
use reqwest::{Client, StatusCode};

const PASSWORD: &str = "correct horse battery staple";

fn browser() -> Client {
    Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("the test client must build")
}

/// A configured instance whose Plex client points at `stub`.
async fn configured(instance: &TempInstance, stub: &PlexTvStub) -> RunningInstance {
    let running = RunningInstance::start_against_plex(instance, &stub.base_url).await;
    let _wizard = Wizard::set_up(&running, "operator", PASSWORD).await;
    running
}

/// Links a plex.tv account to an Afisharr account, as the wizard's Plex step
/// will.
async fn link_plex_account(running: &RunningInstance, plex_account_id: i64) {
    running
        .booted
        .database
        .writer()
        .submit(LinkPlexAccount(plex_account_id))
        .await
        .expect("the link must be writable");
}

struct LinkPlexAccount(i64);

impl WriteOperation for LinkPlexAccount {
    type Output = ();

    async fn execute(self, conn: &mut sqlx::SqliteConnection) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE users SET kind = 'Plex', plex_account_id = ?1, password_hash = NULL
             WHERE username = 'operator'",
        )
        .bind(self.0)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

#[tokio::test]
async fn a_completed_pin_flow_produces_a_working_session() {
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;
    link_plex_account(&running, 4242).await;

    let client = browser();
    let started: serde_json::Value = client
        .post(format!("{}/api/auth/plex/pin", running.base_url))
        .json(&serde_json::json!({ "oauth": false }))
        .send()
        .await
        .expect("the start route must answer")
        .json()
        .await
        .expect("a JSON body");
    let attempt = started["id"].as_str().expect("an attempt id").to_owned();
    assert_eq!(started["code"], "wxyz");

    // Not authorised yet: the answer is pending, not expired. Folding the two
    // would abandon a flow the operator is halfway through (P1).
    let pending: serde_json::Value = client
        .get(format!("{}/api/auth/plex/pin/{attempt}", running.base_url))
        .send()
        .await
        .expect("the poll route must answer")
        .json()
        .await
        .expect("a JSON body");
    assert_eq!(pending["state"], "pending");

    stub.authorize();

    let authorized: serde_json::Value = client
        .get(format!("{}/api/auth/plex/pin/{attempt}", running.base_url))
        .send()
        .await
        .expect("the poll route must answer")
        .json()
        .await
        .expect("a JSON body");
    assert_eq!(authorized["state"], "authorized");
    assert_eq!(authorized["username"], "operator-on-plex");

    // The session works: this is the whole claim.
    let session = client
        .get(format!("{}/api/auth/session", running.base_url))
        .send()
        .await
        .expect("the session route must answer");
    assert_eq!(session.status(), StatusCode::OK);

    // And the token went to `secrets`, sealed, rather than to the login row.
    let sealed: Vec<String> = sqlx::query_scalar("SELECT name FROM secrets")
        .fetch_all(running.booted.database.readers())
        .await
        .expect("the query must run");
    assert!(sealed.contains(&"plex.token".to_owned()), "{sealed:?}");

    let rows: Vec<String> = sqlx::query_scalar("SELECT code FROM plex_pin_logins")
        .fetch_all(running.booted.database.readers())
        .await
        .expect("the query must run");
    assert_eq!(rows, vec!["wxyz".to_owned()]);

    running.stop().await;
    stub.stop().await;
}

#[tokio::test]
async fn the_oauth_variant_shares_the_flow_and_adds_a_sign_in_url() {
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;
    link_plex_account(&running, 4242).await;

    let client = browser();
    let started: serde_json::Value = client
        .post(format!("{}/api/auth/plex/pin", running.base_url))
        .json(&serde_json::json!({
            "oauth": true,
            "forwardUrl": "https://afisharr.example/login",
        }))
        .send()
        .await
        .expect("the start route must answer")
        .json()
        .await
        .expect("a JSON body");

    let url = started["authorizationUrl"]
        .as_str()
        .expect("the OAuth variant carries a sign-in URL");
    assert!(url.starts_with("https://app.plex.tv/auth#"), "{url}");
    assert!(url.contains("code=wxyz"), "{url}");

    // Same polling machinery: the only thing the variant changed is what the
    // operator is shown.
    stub.authorize();
    let attempt = started["id"].as_str().expect("an attempt id");
    let authorized: serde_json::Value = client
        .get(format!("{}/api/auth/plex/pin/{attempt}", running.base_url))
        .send()
        .await
        .expect("the poll route must answer")
        .json()
        .await
        .expect("a JSON body");
    assert_eq!(authorized["state"], "authorized");

    running.stop().await;
    stub.stop().await;
}

#[tokio::test]
async fn a_pin_under_a_mismatched_client_identifier_fails_visibly() {
    // The failure this prevents: a token plex.tv accepts once and refuses
    // afterwards, which reads as an intermittent Plex outage for as long as
    // nobody checks the identifier (PRD §19.6).
    let stub = PlexTvStub::start().await;
    stub.report_client_identifier("some-other-instance");

    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;

    let response = browser()
        .post(format!("{}/api/auth/plex/pin", running.base_url))
        .json(&serde_json::json!({ "oauth": false }))
        .send()
        .await
        .expect("the start route must answer");

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body: serde_json::Value = response.json().await.expect("a JSON body");
    assert_eq!(body["code"], "conflict");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("client identifier"),
        "{body}"
    );

    running.stop().await;
    stub.stop().await;
}

#[tokio::test]
async fn an_unlinked_plex_account_signs_nobody_in_and_stores_nothing() {
    // A completed pin proves somebody holds a plex.tv account. It does not say
    // whose, and an instance that offers Plex sign-in must not hand itself to
    // whoever finishes the exchange first.
    let stub = PlexTvStub::start().await;
    stub.account_is(999_999);

    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;
    // Deliberately not linked.

    let client = browser();
    let started: serde_json::Value = client
        .post(format!("{}/api/auth/plex/pin", running.base_url))
        .json(&serde_json::json!({ "oauth": false }))
        .send()
        .await
        .expect("the start route must answer")
        .json()
        .await
        .expect("a JSON body");
    let attempt = started["id"].as_str().expect("an attempt id").to_owned();

    stub.authorize();
    let refused = client
        .get(format!("{}/api/auth/plex/pin/{attempt}", running.base_url))
        .send()
        .await
        .expect("the poll route must answer");
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);

    // No session.
    assert_eq!(
        client
            .get(format!("{}/api/auth/session", running.base_url))
            .send()
            .await
            .expect("the session route must answer")
            .status(),
        StatusCode::UNAUTHORIZED
    );

    // And no token was stored on the strength of an exchange that proved
    // nothing about who was at the other end.
    let sealed: Vec<String> = sqlx::query_scalar("SELECT name FROM secrets")
        .fetch_all(running.booted.database.readers())
        .await
        .expect("the query must run");
    assert!(!sealed.contains(&"plex.token".to_owned()), "{sealed:?}");

    running.stop().await;
    stub.stop().await;
}

#[tokio::test]
async fn a_plex_sign_in_is_refused_before_the_instance_is_set_up() {
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = RunningInstance::start_against_plex(&instance, &stub.base_url).await;

    let client = browser();
    let response = client
        .post(format!("{}/api/auth/plex/pin", running.base_url))
        .json(&serde_json::json!({ "oauth": false }))
        .send()
        .await
        .expect("the start route must answer");
    let body: serde_json::Value = response.json().await.expect("a JSON body");
    assert_eq!(body["code"], "setupRequired");
    // Dropped before the servers stop, so its idle connection pool does not
    // outlive them and leave the test reported as leaky.
    drop(client);

    // Belt and braces: the account row the flow would have signed in as does
    // not exist either.
    assert!(
        !accounts::admin_exists(running.booted.database.readers())
            .await
            .expect("the query must run")
    );

    running.stop().await;
    stub.stop().await;
}

#[tokio::test]
async fn two_overlapping_polls_produce_one_session_and_not_two() {
    // The interface polls on a timer, so two requests are in flight whenever a
    // poll takes longer than the interval. Both read the attempt as open, both
    // are told `Authorized`, and without an atomic claim both store the token,
    // refresh the account, and mint a session — two valid sessions from one
    // exchange, one of which nobody is holding on purpose.
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;
    link_plex_account(&running, 4242).await;

    let client = browser();
    let started: serde_json::Value = client
        .post(format!("{}/api/auth/plex/pin", running.base_url))
        .json(&serde_json::json!({ "oauth": false }))
        .send()
        .await
        .expect("the start route must answer")
        .json()
        .await
        .expect("a JSON body");
    let attempt = started["id"].as_str().expect("an attempt id").to_owned();

    stub.authorize();

    let poll = || {
        let url = format!("{}/api/auth/plex/pin/{attempt}", running.base_url);
        // A jar of its own per request, so a session cookie set by one does not
        // make the other look like the same browser.
        async move {
            reqwest::Client::new()
                .get(url)
                .send()
                .await
                .expect("the poll route must answer")
        }
    };
    let (first, second) = tokio::join!(poll(), poll());

    let bodies: Vec<serde_json::Value> = vec![
        first.json().await.expect("a JSON body"),
        second.json().await.expect("a JSON body"),
    ];
    let authorized = bodies
        .iter()
        .filter(|body| body["state"] == "authorized")
        .count();
    assert_eq!(authorized, 1, "exactly one poll may sign in: {bodies:?}");

    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(running.booted.database.readers())
        .await
        .expect("the query must run");
    assert_eq!(sessions, 1, "one exchange must produce one session");

    running.stop().await;
    stub.stop().await;
}

#[tokio::test]
async fn polling_is_counted_against_the_provider_budget() {
    // A pin identifier is a public string carrying no credential. A limit spent
    // only when the pin is created leaves anyone who has seen one able to drive
    // unbounded traffic at plex.tv under this instance's client identifier.
    const PROVIDER_ALLOWANCE: usize = 60;

    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;

    let client = browser();
    let started: serde_json::Value = client
        .post(format!("{}/api/auth/plex/pin", running.base_url))
        .json(&serde_json::json!({ "oauth": false }))
        .send()
        .await
        .expect("the start route must answer")
        .json()
        .await
        .expect("a JSON body");
    let attempt = started["id"].as_str().expect("an attempt id").to_owned();

    // Creating the pin spent one, so the rest of the allowance is polls.
    for n in 1..PROVIDER_ALLOWANCE {
        let response = client
            .get(format!("{}/api/auth/plex/pin/{attempt}", running.base_url))
            .send()
            .await
            .expect("the poll route must answer");
        assert_eq!(response.status(), StatusCode::OK, "poll {n}");
    }

    let refused = client
        .get(format!("{}/api/auth/plex/pin/{attempt}", running.base_url))
        .send()
        .await
        .expect("the poll route must answer");
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    let body: serde_json::Value = refused.json().await.expect("a JSON body");
    assert_eq!(body["code"], "rateLimited");

    running.stop().await;
    stub.stop().await;
}

#[tokio::test]
async fn an_unreachable_plex_answers_the_status_the_route_documents() {
    // 502, and not 500. The operation declares an upstream failure, and a
    // generated client that received 500 could not tell an outage it should
    // wait out from a fault it should report.
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;
    stub.stop().await;

    let response = browser()
        .post(format!("{}/api/auth/plex/pin", running.base_url))
        .json(&serde_json::json!({ "oauth": false }))
        .send()
        .await
        .expect("the start route must answer");
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body: serde_json::Value = response.json().await.expect("a JSON body");
    assert_eq!(body["code"], "upstream");

    running.stop().await;
}
