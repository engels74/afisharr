// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Task 1.9: the stream is one authenticated connection, and it never replays.

mod harness;

use harness::{RunningInstance, TempInstance, Wizard};
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
    let _wizard = Wizard::set_up(&running, "operator", PASSWORD).await;

    let client = browser();
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

#[tokio::test]
async fn an_idle_stream_beats_with_an_event_a_listener_can_observe() {
    // `I-UX-9`: a browser's `EventSource` never dispatches an SSE *comment* to
    // a listener, so a keep-alive comment leaves every client's watchdog
    // expiring on a connection that is perfectly healthy. The beat has to be a
    // named event with a body. Slow by construction — the interval is the
    // server's fifteen seconds, and shortening it for the test would be
    // testing a different number than the one that ships.
    let instance = TempInstance::new();
    let (running, client) = signed_in(&instance).await;

    let response = client
        .get(format!("{}/api/stream", running.base_url))
        .send()
        .await
        .expect("the stream route must answer");

    let events = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        first_two_events(response),
    )
    .await
    .expect("an idle stream must beat inside the watchdog window");

    assert!(events[0].contains("\"heartbeatSeconds\":15"), "{events:?}");
    assert!(
        events[1].contains("event: stream"),
        "the beat must be dispatchable: {events:?}"
    );
    assert!(
        events[1].contains("\"heartbeat\":true"),
        "the beat must carry a body a listener can read: {events:?}"
    );

    running.stop().await;
}

#[tokio::test]
async fn a_stop_does_not_wait_for_a_browser_holding_the_stream_open() {
    // The body of an event stream never ends on its own, and a graceful
    // shutdown waits for every response already in flight. Without an end of
    // its own, one open tab holds the process until the container kills it,
    // and `start::run` never reaches `database.close()`.
    let instance = TempInstance::new();
    let (running, client) = signed_in(&instance).await;

    let response = client
        .get(format!("{}/api/stream", running.base_url))
        .send()
        .await
        .expect("the stream route must answer");

    // Read to the end of the opening event, then keep the reader: the body is
    // still in flight when the stop signal arrives, which is the whole case.
    let mut lines = sse_lines(response);
    while let Some(line) = lines
        .next_line()
        .await
        .expect("the stream must be readable")
    {
        if line.is_empty() {
            break;
        }
    }

    let started = std::time::Instant::now();
    tokio::time::timeout(std::time::Duration::from_mins(1), running.stop())
        .await
        .expect("a stop must finish while a client holds the stream open");
    let took = started.elapsed();

    // Inside the drain window rather than at its deadline: the streams are
    // ended, not waited out. A stop that took the whole grace period would
    // mean the cancellation did nothing and the bounded fallback carried it.
    assert!(
        took < std::time::Duration::from_secs(5),
        "the stop waited out the drain window instead of ending the stream: {took:?}"
    );

    drop(lines);
}

/// Reads the first complete event off an SSE response.
async fn first_event(response: reqwest::Response) -> String {
    first_events(response, 1).await.remove(0)
}

/// Reads the opening event and whatever the stream says next.
async fn first_two_events(response: reqwest::Response) -> [String; 2] {
    let mut events = first_events(response, 2).await;
    let second = events.remove(1);
    [events.remove(0), second]
}

/// Reads `wanted` complete events off an SSE response.
///
/// A comment frame — which is what a keep-alive is — starts with `:` and is
/// deliberately kept, so a test that expects a dispatchable event fails loudly
/// rather than blocking until its timeout.
async fn first_events(response: reqwest::Response, wanted: usize) -> Vec<String> {
    let mut lines = sse_lines(response);

    let mut events = Vec::with_capacity(wanted);
    let mut event = String::new();
    while let Some(line) = lines
        .next_line()
        .await
        .expect("the stream must be readable")
    {
        if line.is_empty() {
            if !event.is_empty() {
                events.push(std::mem::take(&mut event));
                if events.len() == wanted {
                    break;
                }
            }
            continue;
        }
        event.push_str(&line);
        event.push('\n');
    }
    events
}

/// The lines of an SSE response, as a reader the caller keeps alive.
///
/// Returned rather than consumed, because one test needs the body to still be
/// in flight while something else happens to the server.
fn sse_lines(
    response: reqwest::Response,
) -> tokio::io::Lines<tokio::io::BufReader<impl tokio::io::AsyncRead>> {
    let reader = tokio_util::io::StreamReader::new(futures_util::TryStreamExt::map_err(
        response.bytes_stream(),
        std::io::Error::other,
    ));
    tokio::io::BufReader::new(reader).lines()
}
