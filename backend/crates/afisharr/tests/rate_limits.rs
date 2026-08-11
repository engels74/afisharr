// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! PRD §21.4.3 — the limit table, enforced rather than declared.
//!
//! Three failures this covers, each of which leaves a limit that reports it is
//! working while doing nothing, or worse: an account lockout keyed by the
//! source address an attacker chooses, an API allowance with nothing spending
//! it, and one allowance shared between the operator and everybody who can
//! reach the instance without a credential.

mod harness;

use afisharr_core::settings::SettingsBody;
use harness::{RunningInstance, TempInstance, Wizard};
use reqwest::{Client, StatusCode};

const PASSWORD: &str = "correct horse battery staple";

/// The account bucket's allowance: five failures, then a lockout (§21.4.3).
const ACCOUNT_ALLOWANCE: usize = 5;

/// The authenticated API allowance: 600 a minute, per credential (§21.4.3).
const API_ALLOWANCE: usize = 600;

/// The allowance for calls carrying no accepted credential.
const ANONYMOUS_ALLOWANCE: usize = 300;

fn client() -> Client {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("the test client must build")
}

/// A client holding a session, as a signed-in browser does.
fn browser() -> Client {
    Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("the test client must build")
}

/// Signs `browser` in, so its jar carries the session cookie.
async fn sign_in(browser: &Client, base_url: &str) {
    let signed_in = browser
        .post(format!("{base_url}/api/auth/login"))
        .json(&serde_json::json!({ "username": "operator", "password": PASSWORD }))
        .send()
        .await
        .expect("the login route must answer");
    assert_eq!(
        signed_in.status(),
        StatusCode::OK,
        "the operator must sign in"
    );
}

/// An instance that trusts the loopback proxy, so a test can vary the address
/// each attempt is attributed to.
async fn behind_a_trusted_proxy(instance: &TempInstance) -> RunningInstance {
    let mut configured = SettingsBody::default();
    configured.http.trust_proxy = vec!["127.0.0.1".to_owned()];
    let running = RunningInstance::start_with(instance, configured).await;
    let _wizard = Wizard::set_up(&running, "operator", PASSWORD).await;
    running
}

/// One sign-in attempt with a wrong password, attributed to `address`.
async fn wrong_password(client: &Client, base_url: &str, address: &str) -> reqwest::Response {
    client
        .post(format!("{base_url}/api/auth/login"))
        .header("x-forwarded-for", address)
        .json(&serde_json::json!({ "username": "operator", "password": "not the password" }))
        .send()
        .await
        .expect("the login route must answer")
}

#[tokio::test]
async fn rotating_the_source_address_does_not_buy_more_attempts_at_one_account() {
    // The attack: five attempts per address against one account, from as many
    // addresses as the attacker can reach the instance from. The account bucket
    // already names the account, so the address must not be part of its key.
    let instance = TempInstance::new();
    let running = behind_a_trusted_proxy(&instance).await;
    let client = client();

    for n in 0..ACCOUNT_ALLOWANCE {
        let response = wrong_password(&client, &running.base_url, &format!("198.51.100.{n}")).await;
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "attempt {n} should have been allowed to fail on the password"
        );
    }

    // A fresh address, and the account is still locked out.
    let refused = wrong_password(&client, &running.base_url, "203.0.113.42").await;
    assert_eq!(
        refused.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "a new source address bought a fresh budget against one account"
    );
    let body: serde_json::Value = refused.json().await.expect("a JSON body");
    assert_eq!(body["code"], "rateLimited");

    // The lockout is a lockout and not a spent window: it is measured in
    // minutes, not in whatever is left of a fifteen-minute window.
    assert!(
        body["retryAfterSeconds"]
            .as_u64()
            .is_some_and(|seconds| seconds > 60),
        "{body}"
    );

    // And the right password is refused too, for as long as it lasts.
    let correct = client
        .post(format!("{}/api/auth/login", running.base_url))
        .header("x-forwarded-for", "203.0.113.43")
        .json(&serde_json::json!({ "username": "operator", "password": PASSWORD }))
        .send()
        .await
        .expect("the login route must answer");
    assert_eq!(correct.status(), StatusCode::TOO_MANY_REQUESTS);

    running.stop().await;
}

#[tokio::test]
async fn a_protected_route_is_counted_against_the_api_allowance() {
    // `Bucket::Api` with nothing spending it is a table entry, not a limit: a
    // caller could drive the database, the filesystem browser, and the stream
    // as fast as the process answers, indefinitely.
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let _wizard = Wizard::set_up(&running, "operator", PASSWORD).await;
    let operator = browser();
    sign_in(&operator, &running.base_url).await;

    for n in 0..API_ALLOWANCE {
        let response = operator
            .get(format!("{}/api/auth/session", running.base_url))
            .send()
            .await
            .expect("the session route must answer");
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "request {n} should have reached the handler"
        );
    }

    let refused = operator
        .get(format!("{}/api/auth/session", running.base_url))
        .send()
        .await
        .expect("the session route must answer");
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(refused.headers().contains_key("retry-after"));
    let body: serde_json::Value = refused.json().await.expect("a JSON body");
    assert_eq!(body["code"], "rateLimited");

    // Health is outside the group and keeps answering: an orchestrator's
    // liveness probe must not be starved by a caller's budget (`I-SEC-8`).
    let health = operator
        .get(format!("{}/api/health", running.base_url))
        .send()
        .await
        .expect("the health route must answer");
    assert_eq!(health.status(), StatusCode::OK);

    running.stop().await;
}

#[tokio::test]
async fn an_unauthenticated_flood_does_not_spend_the_operators_budget() {
    // The denial this closes. `trustProxy` is empty by default, so behind the
    // reverse proxy nearly every deployment runs, all callers resolve to the
    // proxy's one address. One budget counted per address is therefore one
    // budget for everybody — and an unauthenticated client, needing no cookie
    // and no key, could spend it on `/api/auth/login` and hold the dashboard,
    // the filesystem browser, and the stream at 429 for the rest of the window
    // from a single source, with nothing in the answer saying why.
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let _wizard = Wizard::set_up(&running, "operator", PASSWORD).await;

    let operator = browser();
    sign_in(&operator, &running.base_url).await;

    // Everything below carries no credential and arrives from the same address
    // the operator's own browser does, which is the whole difficulty.
    let stranger = client();
    let mut bounded = false;
    for _ in 0..=ANONYMOUS_ALLOWANCE {
        let response = stranger
            .get(format!("{}/api/auth/session", running.base_url))
            .send()
            .await
            .expect("the session route must answer");
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            bounded = true;
            break;
        }
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    assert!(
        bounded,
        "anonymous traffic must be bounded by an allowance of its own"
    );

    let mine = operator
        .get(format!("{}/api/auth/session", running.base_url))
        .send()
        .await
        .expect("the session route must answer");
    assert_eq!(
        mine.status(),
        StatusCode::OK,
        "an unauthenticated flood must not refuse the operator's own interface"
    );

    running.stop().await;
}

#[tokio::test]
async fn an_invented_credential_is_bounded_rather_than_refused_for_ever() {
    // The gap left by counting anonymous traffic on "presents no credential":
    // a caller who attaches a junk bearer token presents one, so the layer
    // steps aside, and the budget that names a credential is never reached
    // because they hold none. Unbounded 401s, at whatever rate they can send.
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let _wizard = Wizard::set_up(&running, "operator", PASSWORD).await;

    let stranger = client();
    let mut bounded = false;
    for n in 0..=ANONYMOUS_ALLOWANCE {
        let response = stranger
            .get(format!("{}/api/settings/sessions", running.base_url))
            .header("authorization", format!("Bearer invented-{n}"))
            .send()
            .await
            .expect("the sessions route must answer");
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            bounded = true;
            break;
        }
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    assert!(
        bounded,
        "a refused credential must be charged to the anonymous allowance"
    );

    running.stop().await;
}
