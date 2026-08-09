// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Task 1.1: the health route answers without a credential, every other route
//! fails in one shape, and every response carries the full header set
//! (`I-SEC-2`).

mod harness;

use harness::{RunningInstance, TempInstance};
use reqwest::{Client, StatusCode};

/// A client that does not follow redirects and keeps its own cookie jar, so a
/// test sees exactly what the server said.
fn client() -> Client {
    Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("the test client must build")
}

#[tokio::test]
async fn the_health_route_answers_two_hundred_with_no_credentials() {
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;

    let response = client()
        .get(format!("{}/api/health", running.base_url))
        .send()
        .await
        .expect("the health route must answer");

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.expect("a JSON body");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["setupCompleted"], false);

    running.stop().await;
}

#[tokio::test]
async fn every_other_route_refuses_on_a_fresh_instance_in_the_one_shape() {
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let client = client();

    // Every route except health and the claim endpoints, driven with no
    // credential on an unclaimed instance (`I-SEC-8`).
    let routes = [
        "/api/auth/session",
        "/api/files/roots",
        "/api/settings/api-keys",
        "/api/settings/sessions",
        "/api/stream",
    ];

    for route in routes {
        let response = client
            .get(format!("{}{route}", running.base_url))
            .send()
            .await
            .unwrap_or_else(|error| panic!("{route} must answer: {error}"));

        assert!(
            response.status().is_client_error(),
            "{route} answered {}",
            response.status()
        );
        let body: serde_json::Value = response
            .json()
            .await
            .unwrap_or_else(|error| panic!("{route} must answer JSON: {error}"));
        assert!(
            body.get("code").is_some() && body.get("message").is_some(),
            "{route} answered {body}"
        );
    }

    running.stop().await;
}

#[tokio::test]
async fn an_unmatched_api_path_answers_the_one_shape_rather_than_an_empty_body() {
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;

    let response = client()
        .get(format!("{}/api/nothing-here", running.base_url))
        .send()
        .await
        .expect("the fallback must answer");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = response.json().await.expect("a JSON body");
    assert_eq!(body["code"], "notFound");

    running.stop().await;
}

#[tokio::test]
async fn every_response_carries_the_full_security_header_set() {
    // `I-SEC-2`: enumerated over the router's own routes, including the 404
    // path and a refusal, because a header applied by a handler is a header
    // that is missing on the one route nobody remembered.
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let client = client();

    let probes = [
        "/api/health",
        "/api/auth/session",
        "/api/nothing-here",
        "/api/setup/status",
        "/",
        "/collections",
    ];

    for probe in probes {
        let response = client
            .get(format!("{}{probe}", running.base_url))
            .send()
            .await
            .unwrap_or_else(|error| panic!("{probe} must answer: {error}"));

        let headers = response.headers();
        for expected in [
            "content-security-policy",
            "x-content-type-options",
            "referrer-policy",
            "permissions-policy",
        ] {
            assert!(
                headers.contains_key(expected),
                "{probe} answered without {expected}"
            );
        }
        assert_eq!(
            headers
                .get("x-content-type-options")
                .and_then(|value| value.to_str().ok()),
            Some("nosniff"),
            "{probe}"
        );
        assert!(
            headers
                .get("content-security-policy")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|policy| policy.contains("frame-ancestors 'none'")),
            "{probe} carried a policy that allows framing"
        );
        // Plaintext, and no trusted proxy: HSTS would ask the browser to
        // refuse the only scheme this instance can be reached on.
        assert!(
            !headers.contains_key("strict-transport-security"),
            "{probe} sent HSTS over plaintext"
        );
    }

    running.stop().await;
}

#[tokio::test]
async fn a_forwarded_https_claim_from_an_untrusted_peer_does_not_produce_hsts() {
    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;

    let response = client()
        .get(format!("{}/api/health", running.base_url))
        .header("x-forwarded-proto", "https")
        .send()
        .await
        .expect("the health route must answer");

    assert!(
        !response.headers().contains_key("strict-transport-security"),
        "an untrusted peer's protocol claim must not be believed"
    );

    running.stop().await;
}

#[tokio::test]
async fn a_forwarded_https_claim_from_a_trusted_peer_produces_hsts() {
    let instance = TempInstance::new();
    let mut configured = afisharr_core::settings::SettingsBody::default();
    configured.http.trust_proxy = vec!["127.0.0.1".to_owned()];
    let running = RunningInstance::start_with(&instance, configured).await;

    let response = client()
        .get(format!("{}/api/health", running.base_url))
        .header("x-forwarded-proto", "https")
        .send()
        .await
        .expect("the health route must answer");

    assert_eq!(
        response
            .headers()
            .get("strict-transport-security")
            .and_then(|value| value.to_str().ok()),
        Some("max-age=31536000; includeSubDomains")
    );

    running.stop().await;
}
