// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `I-SEC-1` — a forged forwarded header never buys a fresh rate-limit budget.
//!
//! The test is the attack rather than the setting: login attempts are driven
//! past the threshold from one peer with a different `X-Forwarded-For` each
//! time, and the lockout must still fire. Then the same shape from a trusted
//! peer, where the forwarded value is the one that counts.

mod harness;

use afisharr_core::settings::SettingsBody;
use harness::{RunningInstance, TempInstance};
use reqwest::{Client, StatusCode};

/// The setup-claim limiter's allowance: five attempts per address per fifteen
/// minutes (PRD §21.4.3). Chosen for this test because it needs no account to
/// exist first, so nothing but the address is being counted.
const SETUP_ALLOWANCE: usize = 5;

fn client() -> Client {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("the test client must build")
}

/// One claim attempt with a wrong token, carrying `forwarded` if given.
async fn attempt(client: &Client, base_url: &str, forwarded: Option<&str>) -> reqwest::Response {
    let mut request = client
        .post(format!("{base_url}/api/setup/claim"))
        .json(&serde_json::json!({ "token": "zzzz-zzzz-zzzz" }));
    if let Some(address) = forwarded {
        request = request.header("x-forwarded-for", address);
    }
    request.send().await.expect("the claim route must answer")
}

#[tokio::test]
async fn a_forged_forwarded_header_from_an_untrusted_peer_is_ignored() {
    // No trusted proxy configured, which is the default: every request is
    // counted against 127.0.0.1 however the header is set.
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let client = client();

    for n in 0..SETUP_ALLOWANCE {
        let response = attempt(&client, &running.base_url, Some(&format!("203.0.113.{n}"))).await;
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "attempt {n} should have been counted and refused on the token"
        );
    }

    // A sixth attempt, with yet another forged address, is limited anyway.
    let limited = attempt(&client, &running.base_url, Some("203.0.113.99")).await;
    assert_eq!(
        limited.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "the forged header bought a fresh budget"
    );
    assert!(limited.headers().contains_key("retry-after"));
    let body: serde_json::Value = limited.json().await.expect("a JSON body");
    assert_eq!(body["code"], "rateLimited");

    running.stop().await;
}

#[tokio::test]
async fn a_trusted_peer_has_its_forwarded_address_counted_instead() {
    let instance = TempInstance::new();
    let mut configured = SettingsBody::default();
    configured.http.trust_proxy = vec!["127.0.0.1".to_owned()];
    let running = RunningInstance::start_with(&instance, configured).await;
    let client = client();

    // Spend the whole allowance for one forwarded address.
    for _ in 0..SETUP_ALLOWANCE {
        let response = attempt(&client, &running.base_url, Some("198.51.100.7")).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let limited = attempt(&client, &running.base_url, Some("198.51.100.7")).await;
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);

    // A different forwarded address behind the same trusted proxy has its own
    // budget, which is the whole point of honouring the header at all.
    let other = attempt(&client, &running.base_url, Some("198.51.100.8")).await;
    assert_eq!(
        other.status(),
        StatusCode::UNAUTHORIZED,
        "a trusted proxy's forwarded address must be counted separately"
    );

    running.stop().await;
}

#[tokio::test]
async fn a_forged_entry_in_front_of_the_proxys_own_is_not_the_one_counted() {
    // The chain a trusted proxy actually produces: whatever the client wrote,
    // then what the proxy saw, appended. Reading the leftmost entry lets the
    // caller pick the address every limit is counted against, on every request,
    // from behind a proxy the operator configured on purpose (`I-SEC-1`).
    let instance = TempInstance::new();
    let mut configured = SettingsBody::default();
    configured.http.trust_proxy = vec!["127.0.0.1".to_owned()];
    let running = RunningInstance::start_with(&instance, configured).await;
    let client = client();

    for n in 0..SETUP_ALLOWANCE {
        let response = attempt(
            &client,
            &running.base_url,
            Some(&format!("9.9.9.{n}, 198.51.100.7")),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "attempt {n}");
    }

    // A different forged entry, the same real one behind it: the budget is
    // spent, because the address that counted is the one the proxy appended.
    let limited = attempt(
        &client,
        &running.base_url,
        Some("203.0.113.99, 198.51.100.7"),
    )
    .await;
    assert_eq!(
        limited.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "the forged leftmost entry bought a fresh budget"
    );

    // And a genuinely different client behind the same proxy still has its own.
    let other = attempt(&client, &running.base_url, Some("198.51.100.8")).await;
    assert_eq!(other.status(), StatusCode::UNAUTHORIZED);

    running.stop().await;
}

#[tokio::test]
async fn the_limiter_answers_before_it_is_told_the_token_is_wrong() {
    // The ordering PRD §21.4.3 fixes: once the address is over its allowance,
    // the answer is the limit and not the token comparison, so a guesser
    // learns nothing more by continuing.
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let client = client();
    let token = running.token.clone().expect("a fresh instance mints one");

    for _ in 0..SETUP_ALLOWANCE {
        let _ = attempt(&client, &running.base_url, None).await;
    }

    let response = client
        .post(format!("{}/api/setup/claim", running.base_url))
        .json(&serde_json::json!({ "token": token }))
        .send()
        .await
        .expect("the claim route must answer");
    assert_eq!(
        response.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "a correct token must not bypass the limit"
    );

    running.stop().await;
}

#[tokio::test]
async fn an_already_claimed_instance_answers_before_the_limiter_is_consulted() {
    // PRD §21.4.3: an operator refreshing the claim page must not spend the
    // attempts they will need once the hold lapses.
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let token = running.token.clone().expect("a fresh instance mints one");

    let holder = Client::builder()
        .cookie_store(true)
        .build()
        .expect("the test client must build");
    let granted = holder
        .post(format!("{}/api/setup/claim", running.base_url))
        .json(&serde_json::json!({ "token": token.clone() }))
        .send()
        .await
        .expect("the claim route must answer");
    assert_eq!(granted.status(), StatusCode::OK);

    // Ten refusals from a second browser, which is twice the allowance.
    let intruder = Client::builder()
        .cookie_store(true)
        .build()
        .expect("the test client must build");
    for n in 0..(SETUP_ALLOWANCE * 2) {
        let response = intruder
            .post(format!("{}/api/setup/claim", running.base_url))
            .json(&serde_json::json!({ "token": token.clone() }))
            .send()
            .await
            .expect("the claim route must answer");
        assert_eq!(
            response.status(),
            StatusCode::CONFLICT,
            "attempt {n} spent a limiter budget it should not have touched"
        );
    }

    running.stop().await;
}
