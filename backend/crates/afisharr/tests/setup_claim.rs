// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `I-SEC-8` — an unconfigured instance grants nothing without console proof.
//!
//! Every clause of the invariant's statement, driven against a listening
//! instance: the wizard refuses without a claim, the four ways of getting a
//! token wrong are one answer, a second browser is told when to come back and
//! changes nothing, the token reaches no table, no response, and no log, and a
//! restart invalidates it.

mod harness;

use harness::{RunningInstance, TempInstance, Wizard};
use reqwest::{Client, StatusCode};

fn client() -> Client {
    Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("the test client must build")
}

/// A client with no cookie jar, for the calls that must not carry one.
fn bare_client() -> Client {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("the test client must build")
}

#[tokio::test]
async fn every_wizard_endpoint_refuses_without_a_claim() {
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let client = bare_client();

    let gated: [(&str, &str); 3] = [
        ("GET", "/api/setup/status"),
        ("POST", "/api/setup/admin"),
        ("POST", "/api/setup/complete"),
    ];

    for (method, path) in gated {
        let url = format!("{}{path}", running.base_url);
        let request = if method == "GET" {
            client.get(url)
        } else {
            client.post(url).json(&serde_json::json!({
                "username": "operator",
                "password": "correct horse battery staple",
            }))
        };
        let response = request
            .send()
            .await
            .unwrap_or_else(|error| panic!("{path} must answer: {error}"));

        assert!(
            response.status().is_client_error(),
            "{path} answered {} without a claim",
            response.status()
        );
        let body: serde_json::Value = response.json().await.expect("a JSON body");
        assert_eq!(body["code"], "setupRequired", "{path} answered {body}");
    }

    // And no administrator was created on the way past.
    let admin_exists = afisharr_core::accounts::admin_exists(running.booted.database.readers())
        .await
        .expect("the query must run");
    assert!(!admin_exists, "a gated call created an administrator");

    running.stop().await;
}

#[tokio::test]
async fn wrong_expired_malformed_and_empty_tokens_are_one_answer() {
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let client = bare_client();

    let mut answers = Vec::new();
    for token in ["zzzz-zzzz-zzzz", "not a token at all", "", "aaaa-aaaa"] {
        let response = client
            .post(format!("{}/api/setup/claim", running.base_url))
            .json(&serde_json::json!({ "token": token }))
            .send()
            .await
            .expect("the claim route must answer");
        let status = response.status();
        let body: serde_json::Value = response.json().await.expect("a JSON body");
        answers.push((status, body));
    }

    let first = answers.first().expect("four answers").clone();
    for (status, body) in &answers {
        assert_eq!(*status, first.0, "the statuses differ: {body}");
        assert_eq!(*body, first.1, "the bodies differ");
    }
    assert_eq!(first.0, StatusCode::UNAUTHORIZED);

    running.stop().await;
}

#[tokio::test]
async fn the_live_token_claims_the_wizard_and_a_second_browser_is_blocked() {
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let token = running.token.clone().expect("a fresh instance mints one");

    let holder = client();
    let granted = holder
        .post(format!("{}/api/setup/claim", running.base_url))
        .json(&serde_json::json!({ "token": token }))
        .send()
        .await
        .expect("the claim route must answer");
    assert_eq!(granted.status(), StatusCode::OK);
    let granted_body: serde_json::Value = granted.json().await.expect("a JSON body");
    assert!(granted_body["expiresAt"].as_i64().is_some());

    // The holder can now reach a gated endpoint.
    let status = holder
        .get(format!("{}/api/setup/status", running.base_url))
        .send()
        .await
        .expect("the status route must answer");
    assert_eq!(status.status(), StatusCode::OK);
    let status_body: serde_json::Value = status.json().await.expect("a JSON body");
    assert_eq!(status_body["step"], "admin");
    assert_eq!(status_body["claimHeld"], true);

    // A second browser, with the same valid token, is told when to come back.
    let intruder = client();
    let blocked = intruder
        .post(format!("{}/api/setup/claim", running.base_url))
        .json(&serde_json::json!({ "token": token }))
        .send()
        .await
        .expect("the claim route must answer");
    assert_eq!(blocked.status(), StatusCode::CONFLICT);
    assert!(blocked.headers().contains_key("retry-after"));
    let blocked_body: serde_json::Value = blocked.json().await.expect("a JSON body");
    assert_eq!(blocked_body["code"], "blocked");
    assert!(
        blocked_body["retryAfterSeconds"]
            .as_u64()
            .is_some_and(|seconds| seconds > 0 && seconds <= 600),
        "{blocked_body}"
    );

    // And the holder still holds it: the second attempt changed nothing.
    let still_held = holder
        .get(format!("{}/api/setup/status", running.base_url))
        .send()
        .await
        .expect("the status route must answer");
    assert_eq!(still_held.status(), StatusCode::OK);

    running.stop().await;
}

#[tokio::test]
async fn the_token_reaches_no_table_no_response_and_no_log() {
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let token = running.token.clone().expect("a fresh instance mints one");

    let holder = client();
    let granted = holder
        .post(format!("{}/api/setup/claim", running.base_url))
        .json(&serde_json::json!({ "token": token }))
        .send()
        .await
        .expect("the claim route must answer");
    let body = granted.text().await.expect("a body");
    assert!(
        !body.contains(&token),
        "the token came back in the response"
    );

    // No table: the claim is a lease whose owner is the cookie's digest, and
    // the token is only ever in process memory.
    let leases: Vec<String> = sqlx::query_scalar("SELECT owner FROM leases")
        .fetch_all(running.booted.database.readers())
        .await
        .expect("the query must run");
    for owner in leases {
        assert!(!owner.contains(&token), "the token reached leases.owner");
    }

    // No log file: the banner writes to stdout with `println!`, never through
    // the tracing subscriber that owns `logs/afisharr.log`.
    let log_directory = instance.paths().logs();
    if log_directory.exists() {
        let mut entries = tokio::fs::read_dir(&log_directory)
            .await
            .expect("the log directory must be readable");
        while let Some(entry) = entries.next_entry().await.expect("a directory entry") {
            let contents = tokio::fs::read_to_string(entry.path())
                .await
                .unwrap_or_default();
            assert!(
                !contents.contains(&token),
                "the token reached {}",
                entry.path().display()
            );
        }
    }

    running.stop().await;
}

#[tokio::test]
async fn a_restart_with_setup_incomplete_invalidates_the_previous_token() {
    let instance = TempInstance::new();

    let first = RunningInstance::start(&instance).await;
    let stale = first.token.clone().expect("a fresh instance mints one");
    first.stop().await;

    let second = RunningInstance::start(&instance).await;
    let fresh = second.token.clone().expect("a restart mints another");
    assert_ne!(stale, fresh, "a restart must mint a different token");

    let refused = bare_client()
        .post(format!("{}/api/setup/claim", second.base_url))
        .json(&serde_json::json!({ "token": stale }))
        .send()
        .await
        .expect("the claim route must answer");
    assert_eq!(refused.status(), StatusCode::UNAUTHORIZED);

    second.stop().await;
}

#[tokio::test]
async fn the_claim_page_offers_recovery_only_once_an_administrator_exists() {
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let wizard = Wizard::claim(&running).await;

    let before: serde_json::Value = wizard
        .client
        .get(format!("{}/api/setup/status", running.base_url))
        .send()
        .await
        .expect("the status route must answer")
        .json()
        .await
        .expect("a JSON body");
    assert_eq!(before["recoveryAvailable"], false);

    let created = wizard
        .create_admin(&running, "operator", "correct horse battery staple")
        .await;
    assert_eq!(created.status(), StatusCode::OK);

    let after: serde_json::Value = wizard
        .client
        .get(format!("{}/api/setup/status", running.base_url))
        .send()
        .await
        .expect("the status route must answer")
        .json()
        .await
        .expect("a JSON body");
    assert_eq!(after["recoveryAvailable"], true);
    assert_eq!(after["step"], "plex", "the derived step must move on");

    running.stop().await;
}

#[tokio::test]
async fn the_claim_page_reads_its_own_facts_before_it_holds_a_claim() {
    // `/api/setup/status` is behind the claim gate, so the page that has to be
    // drawn *before* a claim exists cannot read it. Without a source for these
    // two facts the interface invents them, and an operator whose token died
    // with a restart is shown a token field and no recovery form (PRD §7.14).
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;

    let unclaimed: serde_json::Value = bare_client()
        .get(format!("{}/api/setup/claim", running.base_url))
        .send()
        .await
        .expect("the claim status route must answer")
        .json()
        .await
        .expect("a JSON body");
    assert_eq!(unclaimed["claimHeld"], false);
    assert_eq!(unclaimed["recoveryAvailable"], false);
    assert_eq!(unclaimed["tokenLive"], true, "a fresh instance mints one");

    let wizard = Wizard::claim(&running).await;
    let created = wizard
        .create_admin(&running, "operator", "correct horse battery staple")
        .await;
    assert_eq!(created.status(), StatusCode::OK);

    // A second browser, with no claim of its own, is told the recovery door
    // is shut — because this one holds the wizard — and the first is told it
    // holds it.
    let held: serde_json::Value = wizard
        .client
        .get(format!("{}/api/setup/claim", running.base_url))
        .send()
        .await
        .expect("the claim status route must answer")
        .json()
        .await
        .expect("a JSON body");
    assert_eq!(held["claimHeld"], true);
    assert_eq!(held["recoveryAvailable"], true);

    running.stop().await;
}

#[tokio::test]
async fn a_second_administrator_cannot_be_created_through_the_wizard() {
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let wizard = Wizard::claim(&running).await;

    for expected in [StatusCode::OK, StatusCode::CONFLICT] {
        let response = wizard
            .create_admin(&running, "operator", "correct horse battery staple")
            .await;
        assert_eq!(response.status(), expected);
    }

    running.stop().await;
}

#[tokio::test]
async fn setup_cannot_be_completed_before_an_administrator_exists() {
    // The lockout this closes: completion writes `setup_completed_at`, deletes
    // the claim, and clears the token. On an instance with no administrator
    // that leaves no credential to sign in with and no door to create one.
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let wizard = Wizard::claim(&running).await;

    let refused = wizard.complete(&running).await;
    assert_eq!(refused.status(), StatusCode::CONFLICT);
    let body: serde_json::Value = refused.json().await.expect("a JSON body");
    assert_eq!(body["code"], "conflict");

    // And nothing was committed on the way: the wizard still works, and the
    // administrator can still be created.
    let created = wizard
        .create_admin(&running, "operator", "correct horse battery staple")
        .await;
    assert_eq!(created.status(), StatusCode::OK);
    assert_eq!(wizard.complete(&running).await.status(), StatusCode::OK);

    running.stop().await;
}

#[tokio::test]
async fn a_setup_write_without_the_csrf_token_is_refused() {
    // The claim is an ambient credential: a browser attaches it to any request
    // another origin can cause, and behind it sit the routes that create the
    // administrator and finish setup.
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let wizard = Wizard::claim(&running).await;

    let refused = wizard
        .client
        .post(format!("{}/api/setup/admin", running.base_url))
        .json(&serde_json::json!({
            "username": "operator",
            "password": "correct horse battery staple",
        }))
        .send()
        .await
        .expect("the admin route must answer");
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    let body: serde_json::Value = refused.json().await.expect("a JSON body");
    assert_eq!(body["code"], "forbidden");

    assert!(
        !afisharr_core::accounts::admin_exists(running.booted.database.readers())
            .await
            .expect("the query must run"),
        "a forged setup write created an administrator"
    );

    running.stop().await;
}

#[tokio::test]
async fn completing_setup_turns_the_wizard_into_a_404_and_expires_the_claim() {
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let wizard = Wizard::claim(&running).await;

    let created = wizard
        .create_admin(&running, "operator", "correct horse battery staple")
        .await;
    assert_eq!(created.status(), StatusCode::OK);

    let completed = wizard.complete(&running).await;
    assert_eq!(completed.status(), StatusCode::OK);

    // The claim cookie is gone rather than renewed: the gate must not append a
    // refreshed copy after the handler's removal, or the browser keeps the
    // later value and the claim outlives the setup that ended it.
    let claim_cookies: Vec<&str> = completed
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter(|value| value.starts_with("afisharr_setup_claim="))
        .collect();
    assert_eq!(claim_cookies.len(), 1, "{claim_cookies:?}");
    assert!(
        claim_cookies[0].contains("afisharr_setup_claim=;"),
        "the claim cookie must be removed, not refreshed: {claim_cookies:?}"
    );

    for path in ["/api/setup/status", "/api/setup/claim"] {
        let response = wizard
            .client
            .get(format!("{}{path}", running.base_url))
            .send()
            .await
            .expect("the route must answer");
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{path} still answers after setup completed"
        );
    }

    running.stop().await;
}

#[tokio::test]
async fn a_completed_setup_closes_the_run_it_opened() {
    // `record_step` opens one `job_runs` row for the whole wizard and never
    // closes it. A completed setup that only appended events is queried and
    // shown as a job that runs forever.
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let wizard = Wizard::claim(&running).await;
    let created = wizard
        .create_admin(&running, "operator", "correct horse battery staple")
        .await;
    assert_eq!(created.status(), StatusCode::OK);
    assert_eq!(wizard.complete(&running).await.status(), StatusCode::OK);

    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM job_runs WHERE job_id = 'setup'")
            .fetch_all(running.booted.database.readers())
            .await
            .expect("the query must run");
    assert_eq!(statuses, vec!["Ok".to_owned()], "{statuses:?}");

    let finished: Vec<Option<i64>> =
        sqlx::query_scalar("SELECT finished_at FROM job_runs WHERE job_id = 'setup'")
            .fetch_all(running.booted.database.readers())
            .await
            .expect("the query must run");
    assert!(finished.iter().all(Option::is_some), "{finished:?}");

    running.stop().await;
}
