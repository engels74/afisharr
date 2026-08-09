// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Task 1.9: the stream is one authenticated connection, and it never replays.

mod harness;

use harness::{RunningInstance, TempInstance};
use reqwest::{Client, StatusCode};
use tokio::io::AsyncBufReadExt;

const PASSWORD: &str = "correct horse battery staple";

fn browser() -> Client {
    Client::builder()
        .cookie_store(true)
        .build()
        .expect("the test client must build")
}

async fn signed_in(instance: &TempInstance) -> (RunningInstance, Client) {
    let running = RunningInstance::start(instance).await;
    let token = running.token.clone().expect("a fresh instance mints one");
    let client = browser();

    client
        .post(format!("{}/api/setup/claim", running.base_url))
        .json(&serde_json::json!({ "token": token }))
        .send()
        .await
        .expect("the claim route must answer");
    client
        .post(format!("{}/api/setup/admin", running.base_url))
        .json(&serde_json::json!({ "username": "operator", "password": PASSWORD }))
        .send()
        .await
        .expect("the admin route must answer");
    client
        .post(format!("{}/api/setup/complete", running.base_url))
        .send()
        .await
        .expect("the complete route must answer");
    client
        .post(format!("{}/api/auth/login", running.base_url))
        .json(&serde_json::json!({ "username": "operator", "password": PASSWORD }))
        .send()
        .await
        .expect("the login route must answer");

    (running, client)
}

#[tokio::test]
async fn the_stream_refuses_a_caller_who_is_not_signed_in() {
    let instance = TempInstance::new();
    let (running, _client) = signed_in(&instance).await;

    let anonymous = Client::new();
    let response = anonymous
        .get(format!("{}/api/stream", running.base_url))
        .send()
        .await
        .expect("the stream route must answer");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    running.stop().await;
}

#[tokio::test]
async fn the_stream_opens_by_naming_its_topics_and_its_heartbeat() {
    // The client derives its disconnection watchdog from the interval the
    // server states, rather than from a copy of the number (`I-UX-9`).
    let instance = TempInstance::new();
    let (running, client) = signed_in(&instance).await;

    let response = client
        .get(format!("{}/api/stream", running.base_url))
        .send()
        .await
        .expect("the stream route must answer");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    // The header set applies to the stream too (`I-SEC-2`).
    assert!(response.headers().contains_key("content-security-policy"));

    let opened = first_event(response).await;
    assert!(opened.contains("event: stream"), "{opened}");
    assert!(opened.contains("\"heartbeatSeconds\":15"), "{opened}");
    assert!(opened.contains("\"jobs\""), "{opened}");
    assert!(opened.contains("\"sources\""), "{opened}");

    running.stop().await;
}

#[tokio::test]
async fn a_reconnecting_client_gets_the_same_opening_event_and_no_replay() {
    // A reconnect reconciles by refetching, never by replaying missed events
    // (PRD §9). The stream therefore says the same thing on every connect, and
    // carries nothing that happened while nobody was listening.
    let instance = TempInstance::new();
    let (running, client) = signed_in(&instance).await;

    let first = first_event(
        client
            .get(format!("{}/api/stream", running.base_url))
            .send()
            .await
            .expect("the stream route must answer"),
    )
    .await;

    let second = first_event(
        client
            .get(format!("{}/api/stream", running.base_url))
            .send()
            .await
            .expect("the stream route must answer"),
    )
    .await;

    assert_eq!(first, second, "a reconnect must not replay");

    running.stop().await;
}

/// Reads the first complete event off an SSE response.
async fn first_event(response: reqwest::Response) -> String {
    let stream = response.bytes_stream();
    let reader = tokio_util::io::StreamReader::new(futures_util::TryStreamExt::map_err(
        stream,
        std::io::Error::other,
    ));
    let mut lines = tokio::io::BufReader::new(reader).lines();

    let mut event = String::new();
    while let Some(line) = lines
        .next_line()
        .await
        .expect("the stream must be readable")
    {
        if line.is_empty() && !event.is_empty() {
            break;
        }
        if !line.is_empty() {
            event.push_str(&line);
            event.push('\n');
        }
    }
    event
}
