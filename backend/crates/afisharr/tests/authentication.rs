// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Tasks 1.2 and 1.3: sessions, API keys, and what a database read yields.

mod harness;

use afisharr_core::sessions;
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
async fn signed_out_instance(instance: &TempInstance) -> RunningInstance {
    let running = RunningInstance::start(instance).await;
    let _wizard = Wizard::set_up(&running, "operator", PASSWORD).await;
    running
}

/// Signs `client` in as the administrator and returns its CSRF token.
async fn sign_in(client: &Client, base_url: &str) -> String {
    sign_in_as(client, base_url, "operator").await
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

    // The password and the revocations are one commit, so the new password is
    // in force exactly where the old identifiers are not: a change that landed
    // by halves would leave one of these two assertions false.
    let returning = browser();
    assert_eq!(
        returning
            .post(format!("{}/api/auth/login", running.base_url))
            .json(&serde_json::json!({
                "username": "operator",
                "password": "a different long enough password",
            }))
            .send()
            .await
            .expect("the login route must answer")
            .status(),
        StatusCode::OK,
        "the new password must be the one in force"
    );

    running.stop().await;
}

/// One password change, as a browser makes it.
async fn change_password(
    client: &Client,
    csrf: &str,
    running: &RunningInstance,
    new_password: &str,
) -> (StatusCode, serde_json::Value) {
    let response = client
        .post(format!("{}/api/settings/password", running.base_url))
        .header("x-afisharr-csrf", csrf)
        .json(&serde_json::json!({
            "currentPassword": PASSWORD,
            "newPassword": new_password,
        }))
        .send()
        .await
        .expect("the password route must answer");
    let status = response.status();
    (status, response.json().await.expect("a JSON body"))
}

#[tokio::test]
async fn two_changes_of_one_password_do_not_both_land() {
    // The race: both requests verify the same current password before either
    // writes, because hashing takes long enough for the second to have read the
    // row the first is about to change. Unconditional, the later write wins and
    // revokes the replacement session the earlier one's browser is holding —
    // so the operator who changed the password successfully is signed out by
    // the request that was refused, and the account ends on whichever password
    // committed last.
    const LAPTOP: &str = "the laptop's own new password";
    const DESKTOP: &str = "the desktop's own new password";

    let instance = TempInstance::new();
    let running = signed_out_instance(&instance).await;

    let laptop = browser();
    let laptop_csrf = sign_in(&laptop, &running.base_url).await;
    let desktop = browser();
    let desktop_csrf = sign_in(&desktop, &running.base_url).await;

    let (from_laptop, from_desktop) = tokio::join!(
        change_password(&laptop, &laptop_csrf, &running, LAPTOP),
        change_password(&desktop, &desktop_csrf, &running, DESKTOP),
    );

    let laptop_landed = from_laptop.0 == StatusCode::OK;
    let (winner, winning_password, landed, refused) = if laptop_landed {
        (&laptop, LAPTOP, from_laptop, from_desktop)
    } else {
        (&desktop, DESKTOP, from_desktop, from_laptop)
    };
    assert_eq!(landed.0, StatusCode::OK, "one change must land");
    assert_eq!(
        refused.0,
        StatusCode::CONFLICT,
        "a change of a password that already moved on must be refused, not applied"
    );
    assert_eq!(refused.1["code"], "conflict");

    // The browser whose change landed is still signed in. It holds the
    // replacement session that change issued, and nothing may revoke it
    // afterwards on the strength of a password it never had.
    assert_eq!(
        winner
            .get(format!("{}/api/auth/session", running.base_url))
            .send()
            .await
            .expect("the session route must answer")
            .status(),
        StatusCode::OK,
        "the change that landed must not have its own session revoked"
    );

    // And the account is on that change's password, not on the refused one's.
    for (password, expected) in [
        (winning_password, StatusCode::OK),
        (
            if winning_password == LAPTOP {
                DESKTOP
            } else {
                LAPTOP
            },
            StatusCode::UNAUTHORIZED,
        ),
    ] {
        let returning = browser();
        assert_eq!(
            returning
                .post(format!("{}/api/auth/login", running.base_url))
                .json(&serde_json::json!({ "username": "operator", "password": password }))
                .send()
                .await
                .expect("the login route must answer")
                .status(),
            expected,
            "signing in with '{password}' answered the wrong way"
        );
    }

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

#[tokio::test]
async fn an_account_without_administrator_rights_reaches_none_of_the_admin_surface() {
    // Tier 0 is admin-only (D-007). The schema permits `is_admin = 0`, and
    // without a boundary such an account holds a session this surface accepts
    // and reaches the filesystem browser and the instance's API keys — the
    // documented admin-only surface as ordinary authenticated access.
    let instance = TempInstance::new();
    let running = signed_out_instance(&instance).await;
    create_ordinary_account(&running).await;

    let client = browser();
    let csrf = sign_in_as(&client, &running.base_url, "viewer").await;

    // Self-scoped routes still work: knowing who you are is not a permission.
    assert_eq!(
        client
            .get(format!("{}/api/auth/session", running.base_url))
            .send()
            .await
            .expect("the session route must answer")
            .status(),
        StatusCode::OK
    );

    for route in [
        "/api/files/roots",
        "/api/files?root=assets",
        "/api/settings/api-keys",
        "/api/stream",
    ] {
        let response = client
            .get(format!("{}{route}", running.base_url))
            .send()
            .await
            .unwrap_or_else(|error| panic!("{route} must answer: {error}"));
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "{route} answered an account that does not administer this instance"
        );
        let body: serde_json::Value = response.json().await.expect("a JSON body");
        assert_eq!(body["code"], "forbidden", "{route}");
    }

    // And it cannot mint itself a key to come back with.
    let refused = client
        .post(format!("{}/api/settings/api-keys", running.base_url))
        .header("x-afisharr-csrf", &csrf)
        .json(&serde_json::json!({ "name": "Escalation" }))
        .send()
        .await
        .expect("the key route must answer");
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);

    running.stop().await;
}

/// Adds an enabled account with no administrator rights.
async fn create_ordinary_account(running: &RunningInstance) {
    use afisharr_core::{
        accounts::{CreateUser, CreateUserOutcome},
        identifier::Id,
    };

    let hash = afisharr_core::accounts::hash(PASSWORD.to_owned())
        .await
        .expect("the password must hash");
    let outcome = running
        .booted
        .database
        .writer()
        .submit(CreateUser {
            id: Id::generate(&afisharr_core::time::SystemClock),
            username: "viewer".to_owned(),
            password_hash: hash,
            is_admin: false,
            at: afisharr_core::time::Timestamp::EPOCH,
        })
        .await
        .expect("the write must run")
        .expect("the account must be readable");
    assert!(
        matches!(outcome, CreateUserOutcome::Created(_)),
        "the ordinary account must be created"
    );
}

/// Signs `client` in as `username` and returns the CSRF token it must echo.
async fn sign_in_as(client: &Client, base_url: &str, username: &str) -> String {
    let response = client
        .post(format!("{base_url}/api/auth/login"))
        .json(&serde_json::json!({ "username": username, "password": PASSWORD }))
        .send()
        .await
        .expect("the login route must answer");
    assert_eq!(response.status(), StatusCode::OK, "sign-in must succeed");
    csrf_from(&response).expect("sign-in must set a CSRF cookie")
}

async fn user_id(running: &RunningInstance) -> String {
    afisharr_core::accounts::find_by_username(running.booted.database.readers(), "operator")
        .await
        .expect("the query must run")
        .expect("the administrator exists")
        .id
}
