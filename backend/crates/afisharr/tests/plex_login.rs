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
use harness::{Attempt, CSRF_HEADER, PlexTvStub, RunningInstance, TempInstance, Wizard, browser};
use reqwest::StatusCode;

const PASSWORD: &str = "correct horse battery staple";

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

    let attempt = Attempt::start(&running, false).await;
    assert_eq!(attempt.started["code"], "wxyz");

    // Not authorised yet: the answer is pending, not expired. Folding the two
    // would abandon a flow the operator is halfway through (P1).
    let pending = attempt.poll_body(&running).await;
    assert_eq!(pending["state"], "pending");

    stub.authorize();

    let authorized = attempt.poll_body(&running).await;
    assert_eq!(authorized["state"], "authorized");
    assert_eq!(authorized["username"], "operator-on-plex");
    // The privilege the session actually carries, not an assumed one.
    assert_eq!(authorized["isAdmin"], true);

    // The session works: this is the whole claim.
    let session = attempt
        .client
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

    let attempt = Attempt::start(&running, true).await;

    let url = attempt.started["authorizationUrl"]
        .as_str()
        .expect("the OAuth variant carries a sign-in URL");
    assert!(url.starts_with("https://app.plex.tv/auth#"), "{url}");
    assert!(url.contains("code=wxyz"), "{url}");

    // Same polling machinery: the only thing the variant changed is what the
    // operator is shown.
    stub.authorize();
    let authorized = attempt.poll_body(&running).await;
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

    let attempt = Attempt::start(&running, false).await;

    stub.authorize();
    let refused = attempt.poll(&running).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);

    // No session.
    assert_eq!(
        attempt
            .client
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

    let attempt = Attempt::start(&running, false).await;

    stub.authorize();

    let poll = || {
        let url = format!("{}/api/auth/plex/pin/{}", running.base_url, attempt.id());
        // A jar of its own per request, so a session cookie set by one does not
        // make the other look like the same browser. Both still present the
        // attempt's own cookie and token: this is one browser polling twice,
        // which is exactly the race the atomic claim exists for.
        let cookie = format!("afisharr_plex_pin={}", attempt.id());
        let csrf = attempt.csrf.clone();
        async move {
            reqwest::Client::new()
                .post(url)
                .header(
                    reqwest::header::COOKIE,
                    format!("{cookie}; afisharr_csrf={csrf}"),
                )
                .header(CSRF_HEADER, csrf.clone())
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

    let attempt = Attempt::start(&running, false).await;

    // Creating the pin spent one, so the rest of the allowance is polls.
    for n in 1..PROVIDER_ALLOWANCE {
        let response = attempt.poll(&running).await;
        assert_eq!(response.status(), StatusCode::OK, "poll {n}");
    }

    let refused = attempt.poll(&running).await;
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

#[tokio::test]
async fn an_attempt_cannot_be_completed_by_a_browser_that_did_not_start_it() {
    // The login forgery this closes. An attempt identifier is a public string
    // — the account that started the exchange knows it, and it is read out
    // loud on the pin screen. Completing an attempt mints a session, so a
    // second browser that can complete somebody else's attempt is handed a
    // session for somebody else's account.
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;
    link_plex_account(&running, 4242).await;

    let attempt = Attempt::start(&running, false).await;
    stub.authorize();

    // A different browser, knowing the identifier and nothing else.
    let bystander = browser();
    let refused = bystander
        .post(format!(
            "{}/api/auth/plex/pin/{}",
            running.base_url,
            attempt.id()
        ))
        .send()
        .await
        .expect("the poll route must answer");
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);

    assert_eq!(
        bystander
            .get(format!("{}/api/auth/session", running.base_url))
            .send()
            .await
            .expect("the session route must answer")
            .status(),
        StatusCode::UNAUTHORIZED,
        "a refused completion must not have signed anybody in"
    );

    // And the attempt is still there for the browser that owns it.
    let authorized = attempt.poll_body(&running).await;
    assert_eq!(authorized["state"], "authorized");

    drop(bystander);
    running.stop().await;
    stub.stop().await;
}

#[tokio::test]
async fn completing_an_attempt_is_judged_by_the_cross_site_check() {
    // The attempt cookie is an ambient credential, so the request that turns a
    // finished exchange into a session is one a browser can be made to send.
    // It is judged like every other credentialled write.
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;
    link_plex_account(&running, 4242).await;

    let attempt = Attempt::start(&running, false).await;
    stub.authorize();

    let forged = attempt
        .client
        .post(format!(
            "{}/api/auth/plex/pin/{}",
            running.base_url,
            attempt.id()
        ))
        .header(reqwest::header::ORIGIN, "https://evil.example")
        .header(CSRF_HEADER, &attempt.csrf)
        .send()
        .await
        .expect("the poll route must answer");
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);

    let without_token = attempt
        .client
        .post(format!(
            "{}/api/auth/plex/pin/{}",
            running.base_url,
            attempt.id()
        ))
        .send()
        .await
        .expect("the poll route must answer");
    assert_eq!(without_token.status(), StatusCode::FORBIDDEN);

    running.stop().await;
    stub.stop().await;
}

#[tokio::test]
async fn a_viewer_signing_in_keeps_the_integration_token_and_its_own_privilege() {
    // `plex.token` is one credential for the whole instance and it is what
    // every server operation runs under. A linked account that administers
    // nothing signing in must not replace it with their own lower-privilege
    // token — that breaks Plex for everybody at the moment a viewer signs in.
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;

    // The administrator signs in first, so the integration credential exists.
    link_plex_account(&running, 4242).await;
    let admin_attempt = Attempt::start(&running, false).await;
    stub.authorize();
    let as_admin = admin_attempt.poll_body(&running).await;
    assert_eq!(as_admin["isAdmin"], true);

    let owner_token: String =
        sqlx::query_scalar("SELECT hex(ciphertext) FROM secrets WHERE name = ?1")
            .bind("plex.token")
            .fetch_one(running.booted.database.readers())
            .await
            .expect("the integration credential must have been stored");

    // Now a viewer: a second linked account, active, and not an administrator.
    add_viewer(&running, 777).await;
    stub.account_is(777);
    stub.username_is("viewer-on-plex");
    stub.authorize();

    let viewer_attempt = Attempt::start(&running, false).await;
    let as_viewer = viewer_attempt.poll_body(&running).await;
    assert_eq!(as_viewer["state"], "authorized");
    assert_eq!(
        as_viewer["isAdmin"], false,
        "the poll must report the privilege the session carries"
    );

    // The session is real.
    assert_eq!(
        viewer_attempt
            .client
            .get(format!("{}/api/auth/session", running.base_url))
            .send()
            .await
            .expect("the session route must answer")
            .status(),
        StatusCode::OK
    );

    let after: String = sqlx::query_scalar("SELECT hex(ciphertext) FROM secrets WHERE name = ?1")
        .bind("plex.token")
        .fetch_one(running.booted.database.readers())
        .await
        .expect("the integration credential must still be there");
    assert_eq!(
        after, owner_token,
        "a viewer's sign-in must not overwrite the integration credential"
    );

    running.stop().await;
    stub.stop().await;
}

/// Adds a linked, active, non-administering Plex account.
async fn add_viewer(running: &RunningInstance, plex_account_id: i64) {
    struct AddViewer(i64);

    impl WriteOperation for AddViewer {
        type Output = ();

        async fn execute(self, conn: &mut sqlx::SqliteConnection) -> Result<(), sqlx::Error> {
            sqlx::query(
                "INSERT INTO users
                   (id, kind, username, password_hash, plex_account_id, is_admin,
                    created_at, updated_at)
                 VALUES ('01JVIEWER000000000000000000', 'Plex', 'viewer', NULL, ?1, 0, 0, 0)",
            )
            .bind(self.0)
            .execute(&mut *conn)
            .await?;
            Ok(())
        }
    }

    running
        .booted
        .database
        .writer()
        .submit(AddViewer(plex_account_id))
        .await
        .expect("the viewer must be writable");
}
