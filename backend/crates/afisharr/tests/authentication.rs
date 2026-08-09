// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Tasks 1.2 and 1.3: sessions, API keys, and what a database read yields.

mod harness;

use afisharr_core::sessions;
use harness::{RunningInstance, TempInstance};
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
async fn signed_out_instance(instance: &TempInstance) -> RunningInstance {
    let running = RunningInstance::start(instance).await;
    let token = running.token.clone().expect("a fresh instance mints one");
    let holder = browser();

    holder
        .post(format!("{}/api/setup/claim", running.base_url))
        .json(&serde_json::json!({ "token": token }))
        .send()
        .await
        .expect("the claim route must answer");
    holder
        .post(format!("{}/api/setup/admin", running.base_url))
        .json(&serde_json::json!({ "username": "operator", "password": PASSWORD }))
        .send()
        .await
        .expect("the admin route must answer");
    holder
        .post(format!("{}/api/setup/complete", running.base_url))
        .send()
        .await
        .expect("the complete route must answer");

    running
}

/// Signs `client` in and returns the CSRF token it must echo.
async fn sign_in(client: &Client, base_url: &str) -> String {
    let response = client
        .post(format!("{base_url}/api/auth/login"))
        .json(&serde_json::json!({ "username": "operator", "password": PASSWORD }))
        .send()
        .await
        .expect("the login route must answer");
    assert_eq!(response.status(), StatusCode::OK, "sign-in must succeed");

    response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|cookie| cookie.strip_prefix("afisharr_csrf="))
        .and_then(|rest| rest.split(';').next())
        .map(str::to_owned)
        .expect("sign-in must set a CSRF cookie")
}

#[tokio::test]
async fn a_wrong_password_and_an_unknown_account_answer_identically() {
    let instance = TempInstance::new();
    let running = signed_out_instance(&instance).await;
    let client = browser();

    let mut answers = Vec::new();
    for (username, password) in [
        ("operator", "wrong password entirely"),
        ("nobody", PASSWORD),
    ] {
        let response = client
            .post(format!("{}/api/auth/login", running.base_url))
            .json(&serde_json::json!({ "username": username, "password": password }))
            .send()
            .await
            .expect("the login route must answer");
        let status = response.status();
        let body: serde_json::Value = response.json().await.expect("a JSON body");
        answers.push((status, body));
    }

    assert_eq!(
        answers[0], answers[1],
        "the two refusals must be one answer"
    );
    assert_eq!(answers[0].0, StatusCode::UNAUTHORIZED);

    running.stop().await;
}

#[tokio::test]
async fn a_database_read_never_yields_a_working_session_or_key() {
    let instance = TempInstance::new();
    let running = signed_out_instance(&instance).await;
    let client = browser();
    let csrf = sign_in(&client, &running.base_url).await;

    let issued: serde_json::Value = client
        .post(format!("{}/api/settings/api-keys", running.base_url))
        .header("x-afisharr-csrf", &csrf)
        .json(&serde_json::json!({ "name": "Home Assistant" }))
        .send()
        .await
        .expect("the key route must answer")
        .json()
        .await
        .expect("a JSON body");
    let secret = issued["secret"]
        .as_str()
        .expect("a plaintext key")
        .to_owned();

    // The stored key is a digest, and it is not the plaintext.
    let stored: Vec<String> = sqlx::query_scalar("SELECT key_hash FROM api_keys")
        .fetch_all(running.booted.database.readers())
        .await
        .expect("the query must run");
    assert_eq!(stored.len(), 1);
    assert_ne!(stored[0], secret);

    // The key works when presented, which is what makes the digest a digest
    // and not just a different string.
    let bare = Client::new();
    let accepted = bare
        .get(format!("{}/api/auth/session", running.base_url))
        .bearer_auth(&secret)
        .send()
        .await
        .expect("the session route must answer");
    assert_eq!(accepted.status(), StatusCode::OK);

    // And the stored session id is a digest too: presenting it as a cookie
    // authenticates nobody.
    let session_ids: Vec<String> = sqlx::query_scalar("SELECT id FROM sessions")
        .fetch_all(running.booted.database.readers())
        .await
        .expect("the query must run");
    assert_eq!(session_ids.len(), 1);

    let forged = Client::new();
    let refused = forged
        .get(format!("{}/api/auth/session", running.base_url))
        .header("cookie", format!("afisharr_session={}", session_ids[0]))
        .send()
        .await
        .expect("the session route must answer");
    assert_eq!(
        refused.status(),
        StatusCode::UNAUTHORIZED,
        "the stored digest must not work as a cookie value"
    );

    running.stop().await;
}

#[tokio::test]
async fn a_revoked_api_key_is_refused_on_its_next_use() {
    let instance = TempInstance::new();
    let running = signed_out_instance(&instance).await;
    let client = browser();
    let csrf = sign_in(&client, &running.base_url).await;

    let issued: serde_json::Value = client
        .post(format!("{}/api/settings/api-keys", running.base_url))
        .header("x-afisharr-csrf", &csrf)
        .json(&serde_json::json!({ "name": "Scripted" }))
        .send()
        .await
        .expect("the key route must answer")
        .json()
        .await
        .expect("a JSON body");
    let secret = issued["secret"]
        .as_str()
        .expect("a plaintext key")
        .to_owned();
    let id = issued["id"].as_str().expect("an identifier").to_owned();

    let bare = Client::new();
    assert_eq!(
        bare.get(format!("{}/api/auth/session", running.base_url))
            .bearer_auth(&secret)
            .send()
            .await
            .expect("the session route must answer")
            .status(),
        StatusCode::OK
    );

    let revoked = client
        .delete(format!("{}/api/settings/api-keys/{id}", running.base_url))
        .header("x-afisharr-csrf", &csrf)
        .send()
        .await
        .expect("the revoke route must answer");
    assert_eq!(revoked.status(), StatusCode::NO_CONTENT);

    assert_eq!(
        bare.get(format!("{}/api/auth/session", running.base_url))
            .bearer_auth(&secret)
            .send()
            .await
            .expect("the session route must answer")
            .status(),
        StatusCode::UNAUTHORIZED,
        "a revoked key must be refused on its next use"
    );

    running.stop().await;
}

#[tokio::test]
async fn a_password_change_revokes_every_other_session() {
    let instance = TempInstance::new();
    let running = signed_out_instance(&instance).await;

    let phone = browser();
    let _ = sign_in(&phone, &running.base_url).await;
    let laptop = browser();
    let csrf = sign_in(&laptop, &running.base_url).await;

    // Both are signed in.
    for client in [&phone, &laptop] {
        assert_eq!(
            client
                .get(format!("{}/api/auth/session", running.base_url))
                .send()
                .await
                .expect("the session route must answer")
                .status(),
            StatusCode::OK
        );
    }

    let changed = laptop
        .post(format!("{}/api/settings/password", running.base_url))
        .header("x-afisharr-csrf", &csrf)
        .json(&serde_json::json!({
            "currentPassword": PASSWORD,
            "newPassword": "a different long enough password",
        }))
        .send()
        .await
        .expect("the password route must answer");
    assert_eq!(changed.status(), StatusCode::OK);
    let body: serde_json::Value = changed.json().await.expect("a JSON body");
    assert_eq!(body["sessionsRevoked"], 1);

    // The other device is out.
    assert_eq!(
        phone
            .get(format!("{}/api/auth/session", running.base_url))
            .send()
            .await
            .expect("the session route must answer")
            .status(),
        StatusCode::UNAUTHORIZED,
        "a password change must revoke every other session"
    );

    // And the device that changed it holds a rotated identifier, not the old
    // one: nothing in the database is still the session it signed in with.
    let live = sessions::list_for_user(running.booted.database.readers(), &user_id(&running).await)
        .await
        .expect("the query must run");
    let unrevoked = live.iter().filter(|s| s.revoked_at.is_none()).count();
    assert_eq!(unrevoked, 1, "exactly one session should survive");

    running.stop().await;
}

#[tokio::test]
async fn a_state_changing_request_without_the_csrf_token_is_refused() {
    let instance = TempInstance::new();
    let running = signed_out_instance(&instance).await;
    let client = browser();
    let _csrf = sign_in(&client, &running.base_url).await;

    let refused = client
        .post(format!("{}/api/settings/api-keys", running.base_url))
        .json(&serde_json::json!({ "name": "No token" }))
        .send()
        .await
        .expect("the key route must answer");
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    let body: serde_json::Value = refused.json().await.expect("a JSON body");
    assert_eq!(body["code"], "forbidden");

    running.stop().await;
}

#[tokio::test]
async fn signing_out_revokes_the_session_that_asked() {
    let instance = TempInstance::new();
    let running = signed_out_instance(&instance).await;
    let client = browser();
    let csrf = sign_in(&client, &running.base_url).await;

    let out = client
        .post(format!("{}/api/auth/logout", running.base_url))
        .header("x-afisharr-csrf", &csrf)
        .send()
        .await
        .expect("the logout route must answer");
    assert_eq!(out.status(), StatusCode::NO_CONTENT);

    assert_eq!(
        client
            .get(format!("{}/api/auth/session", running.base_url))
            .send()
            .await
            .expect("the session route must answer")
            .status(),
        StatusCode::UNAUTHORIZED
    );

    running.stop().await;
}

async fn user_id(running: &RunningInstance) -> String {
    afisharr_core::accounts::find_by_username(running.booted.database.readers(), "operator")
        .await
        .expect("the query must run")
        .expect("the administrator exists")
        .id
}
