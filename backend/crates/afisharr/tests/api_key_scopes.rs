// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Task 1.3: an API key reaches what it was issued for, and nothing else.
//!
//! The escalation this covers. A key carried no answer to "what may this do",
//! so the guard answered it from whoever issued it: a key an administrator
//! created *was* an administrator, on every route, including the one that
//! creates keys. An operator handing one integration a token to browse their
//! files handed it the Plex connection, every session, and the ability to mint
//! a second credential that outlives revoking the first — so revoking the leaked
//! key would not have ended the access.

mod harness;

use harness::{RunningInstance, TempInstance, Wizard, csrf_from};
use reqwest::{Client, StatusCode};

const PASSWORD: &str = "correct horse battery staple";

fn browser() -> Client {
    Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("the test client must build")
}

/// A claimed instance with an administrator and setup finished.
async fn instance_with_an_operator(instance: &TempInstance) -> (RunningInstance, Client, String) {
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

/// Issues a key with `scopes` and returns its plaintext.
async fn issue(
    operator: &Client,
    base_url: &str,
    csrf: &str,
    scopes: &[&str],
) -> serde_json::Value {
    operator
        .post(format!("{base_url}/api/settings/api-keys"))
        .header("x-afisharr-csrf", csrf)
        .json(&serde_json::json!({ "name": "An integration", "scopes": scopes }))
        .send()
        .await
        .expect("the key route must answer")
        .json()
        .await
        .expect("a JSON body")
}

#[tokio::test]
async fn a_key_issued_to_read_files_cannot_mint_another_key() {
    // The escalation, exactly: revoking a leaked key ends nothing if the key
    // could issue its own successor before anybody noticed.
    let instance = TempInstance::new();
    let (running, operator, csrf) = instance_with_an_operator(&instance).await;
    let issued = issue(&operator, &running.base_url, &csrf, &["files:read"]).await;
    let secret = issued["secret"].as_str().expect("a plaintext key");

    let script = Client::new();
    let refused = script
        .post(format!("{}/api/settings/api-keys", running.base_url))
        .bearer_auth(secret)
        .json(&serde_json::json!({ "name": "Successor", "scopes": ["keys:manage"] }))
        .send()
        .await
        .expect("the key route must answer");
    assert_eq!(
        refused.status(),
        StatusCode::FORBIDDEN,
        "a key issued to read files minted a second key"
    );
    let body: serde_json::Value = refused.json().await.expect("a JSON body");
    assert_eq!(body["code"], "forbidden");

    // Only one key exists, which is the fact the status code stands for.
    let listed: serde_json::Value = operator
        .get(format!("{}/api/settings/api-keys", running.base_url))
        .send()
        .await
        .expect("the list route must answer")
        .json()
        .await
        .expect("a JSON body");
    assert_eq!(listed.as_array().map(Vec::len), Some(1), "{listed}");

    running.stop().await;
}

#[tokio::test]
async fn a_key_reaches_the_route_its_scope_names_and_no_other() {
    let instance = TempInstance::new();
    let (running, operator, csrf) = instance_with_an_operator(&instance).await;
    let issued = issue(&operator, &running.base_url, &csrf, &["files:read"]).await;
    let secret = issued["secret"].as_str().expect("a plaintext key");
    assert_eq!(issued["scopes"], serde_json::json!(["files:read"]));

    let script = Client::new();
    let reached = script
        .get(format!("{}/api/files/roots", running.base_url))
        .bearer_auth(secret)
        .send()
        .await
        .expect("the roots route must answer");
    assert_eq!(
        reached.status(),
        StatusCode::OK,
        "the scope the key was issued with must work"
    );

    // Everything else on the surface, refused. Each of these was reachable
    // when the guard read the creator's rights instead of the key's scopes.
    for route in [
        "/api/stream",
        "/api/settings/sessions",
        "/api/settings/api-keys",
    ] {
        let response = script
            .get(format!("{}{route}", running.base_url))
            .bearer_auth(secret)
            .send()
            .await
            .unwrap_or_else(|error| panic!("{route} must answer: {error}"));
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "{route} answered a key that was not issued for it"
        );
    }

    running.stop().await;
}

#[tokio::test]
async fn a_refusal_names_the_scope_the_key_would_have_needed() {
    // The caller is a script, and its author is reading a log rather than the
    // interface. "Forbidden" alone leaves them guessing which of five
    // capabilities to re-issue the key with.
    let instance = TempInstance::new();
    let (running, operator, csrf) = instance_with_an_operator(&instance).await;
    let issued = issue(&operator, &running.base_url, &csrf, &["files:read"]).await;
    let secret = issued["secret"].as_str().expect("a plaintext key");

    let body: serde_json::Value = Client::new()
        .get(format!("{}/api/stream", running.base_url))
        .bearer_auth(secret)
        .send()
        .await
        .expect("the stream route must answer")
        .json()
        .await
        .expect("a JSON body");
    assert_eq!(body["code"], "forbidden");
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|message| message.contains("events:read")),
        "{body}"
    );

    running.stop().await;
}

#[tokio::test]
async fn a_key_cannot_be_issued_without_saying_what_it_may_do() {
    // No default, and deliberately no "everything": a key issued without
    // anybody choosing is how the whole instance ends up in a config file.
    let instance = TempInstance::new();
    let (running, operator, csrf) = instance_with_an_operator(&instance).await;

    for scopes in [serde_json::json!([]), serde_json::json!(["files:write"])] {
        let refused = operator
            .post(format!("{}/api/settings/api-keys", running.base_url))
            .header("x-afisharr-csrf", &csrf)
            .json(&serde_json::json!({ "name": "Unscoped", "scopes": scopes }))
            .send()
            .await
            .expect("the key route must answer");
        assert_eq!(refused.status(), StatusCode::BAD_REQUEST, "{scopes}");
        let body: serde_json::Value = refused.json().await.expect("a JSON body");
        assert_eq!(body["code"], "invalid");
        assert_eq!(body["pointer"], "/scopes");
    }

    // And a body with no `scopes` field at all is not an unscoped key either.
    let refused = operator
        .post(format!("{}/api/settings/api-keys", running.base_url))
        .header("x-afisharr-csrf", &csrf)
        .json(&serde_json::json!({ "name": "Unscoped" }))
        .send()
        .await
        .expect("the key route must answer");
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);

    running.stop().await;
}

#[tokio::test]
async fn a_session_reaches_everything_the_operator_reaches() {
    // A scope narrows a key below its account. The operator sitting at the
    // interface is the account, and nothing here may have narrowed them.
    let instance = TempInstance::new();
    let (running, operator, _csrf) = instance_with_an_operator(&instance).await;

    for route in [
        "/api/files/roots",
        "/api/settings/sessions",
        "/api/settings/api-keys",
    ] {
        let response = operator
            .get(format!("{}{route}", running.base_url))
            .send()
            .await
            .unwrap_or_else(|error| panic!("{route} must answer: {error}"));
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{route} refused the operator's own browser"
        );
    }

    running.stop().await;
}
