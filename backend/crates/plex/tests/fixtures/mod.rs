// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A server that answers one hand-rolled body, and records what it was asked.
//!
//! Task 2.1 requires every call to be exercised against a fixture response
//! *before* the fake exists, and the two prove different things: the fixture
//! proves this client sends the request a real Plex expects and reads the body
//! a real Plex sends, and the fake proves the client survives a server that
//! misbehaves. A fixture built by serialising the client's own types would
//! prove neither, so every body here is written out by hand.

use std::sync::{Arc, Mutex};

use afisharr_plex::{
    identity::ClientIdentity,
    server::{PlexServerClient, ServerAddress, ServerToken},
};
use afisharr_sources::outbound::OutboundClient;
use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, Uri},
    response::IntoResponse,
    routing::any,
};

/// One request the client made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    /// The method.
    pub method: String,
    /// The path, without the query.
    pub path: String,
    /// The query string exactly as it went on the wire.
    pub query: String,
    /// The `X-Plex-Token` header, if one was sent.
    pub token: Option<String>,
    /// The `X-Plex-Client-Identifier` header, if one was sent.
    pub client_identifier: Option<String>,
    /// The `Accept` header, if one was sent.
    pub accept: Option<String>,
    /// The `Content-Type` header, if one was sent.
    pub content_type: Option<String>,
    /// How many bytes of body arrived.
    pub body_len: usize,
}

impl Recorded {
    /// The value of one query parameter, decoded.
    ///
    /// Decoded because the assertion is about what the server will read, and a
    /// test that compared the encoded form would pass on a client that encoded
    /// the wrong character.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<String> {
        serde_urlencoded::from_str::<Vec<(String, String)>>(&self.query)
            .ok()?
            .into_iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }

    /// Every value of one repeated query parameter, decoded.
    #[must_use]
    pub fn params(&self, name: &str) -> Vec<String> {
        serde_urlencoded::from_str::<Vec<(String, String)>>(&self.query)
            .unwrap_or_default()
            .into_iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value)
            .collect()
    }
}

#[derive(Debug)]
struct Script {
    body: String,
    status: u16,
    seen: Mutex<Vec<Recorded>>,
}

/// A server answering one fixture body.
pub struct FixtureServer {
    base_url: String,
    script: Arc<Script>,
    task: tokio::task::JoinHandle<()>,
}

impl FixtureServer {
    /// Starts a server that answers `body` with 200 to everything.
    pub async fn answering(body: &str) -> Self {
        Self::answering_with(200, body).await
    }

    /// Starts a server that answers `body` under `status` to everything.
    pub async fn answering_with(status: u16, body: &str) -> Self {
        let script = Arc::new(Script {
            body: body.to_owned(),
            status,
            seen: Mutex::new(Vec::new()),
        });
        let app = Router::new()
            .fallback(any(record))
            .with_state(Arc::clone(&script));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a loopback port must be bindable");
        let address = listener.local_addr().expect("a bound address");
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Self {
            base_url: format!("http://{address}"),
            script,
            task,
        }
    }

    /// A client pointed at this server.
    pub fn client(&self) -> PlexServerClient {
        PlexServerClient::new(
            OutboundClient::new("afisharr/test").expect("the transport must build"),
            ClientIdentity::new("01JTESTCLIENT", "Test Instance", "0.1.0")
                .expect("a valid identity"),
            ServerAddress::parse(&self.base_url).expect("a valid address"),
            ServerToken::new("test-plex-token").expect("a header-safe token"),
        )
    }

    /// The one request this server saw.
    ///
    /// Panics when it saw none or more than one, because every test here makes
    /// exactly one call and a second would mean the client is doing something
    /// the test did not ask for.
    pub fn only_request(&self) -> Recorded {
        let seen = self
            .script
            .seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(seen.len(), 1, "expected exactly one request, saw {seen:#?}");
        seen[0].clone()
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn record(
    State(script): State<Arc<Script>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let text = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    script
        .seen
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(Recorded {
            method: method.to_string(),
            path: uri.path().to_owned(),
            query: uri.query().unwrap_or_default().to_owned(),
            token: text("x-plex-token"),
            client_identifier: text("x-plex-client-identifier"),
            accept: text("accept"),
            content_type: text("content-type"),
            body_len: body.len(),
        });
    (
        axum::http::StatusCode::from_u16(script.status)
            .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        script.body.clone(),
    )
}
