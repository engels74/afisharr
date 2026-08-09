// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A stand-in for plex.tv, just large enough to drive a PIN exchange.
//!
//! Deliberately not the adversarial fake: D-036's fake is Phase 2 work, has a
//! fidelity contract, and is what every phase from Phase 4 onward tests
//! against. This answers three endpoints so Phase 1's login can be driven end
//! to end, and it will be deleted when the real fake arrives.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde_json::json;

/// What the stub will say when it is asked.
#[derive(Debug)]
pub struct PlexTvScript {
    /// The client identifier the pin is reported as having been created under.
    pub client_identifier: Mutex<Option<String>>,
    /// Whether the pin has been authorised yet.
    pub authorized: AtomicBool,
    /// The plex.tv account the token resolves to.
    pub account_id: Mutex<i64>,
    /// The account's username.
    pub username: Mutex<String>,
}

impl Default for PlexTvScript {
    fn default() -> Self {
        Self {
            client_identifier: Mutex::new(None),
            authorized: AtomicBool::new(false),
            account_id: Mutex::new(4242),
            username: Mutex::new("operator-on-plex".to_owned()),
        }
    }
}

/// A running stand-in.
pub struct PlexTvStub {
    /// The API root to point the client at.
    pub base_url: String,
    /// What it will say.
    pub script: Arc<PlexTvScript>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl PlexTvStub {
    /// Starts the stub on a port the operating system chooses.
    pub async fn start() -> Self {
        let script = Arc::new(PlexTvScript::default());
        let app = Router::new()
            .route("/pins", post(create_pin))
            .route("/pins/{id}", get(poll_pin))
            .route("/user", get(account))
            .with_state(Arc::clone(&script));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("the stub must bind");
        let address = listener.local_addr().expect("a bound address");

        let (shutdown, stop) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stop.await;
                })
                .await;
        });

        Self {
            base_url: format!("http://{address}"),
            script,
            shutdown: Some(shutdown),
            task: Some(task),
        }
    }

    /// Says the operator has finished signing in.
    pub fn authorize(&self) {
        self.script.authorized.store(true, Ordering::SeqCst);
    }

    /// Says the pin was created under a different client identifier.
    pub fn report_client_identifier(&self, identifier: &str) {
        *self
            .script
            .client_identifier
            .lock()
            .expect("the stub's lock is not poisoned") = Some(identifier.to_owned());
    }

    /// Says the account holder goes by this name on plex.tv.
    pub fn username_is(&self, username: &str) {
        username.clone_into(
            &mut self
                .script
                .username
                .lock()
                .expect("the stub's lock is not poisoned"),
        );
    }

    /// Says the token belongs to this plex.tv account.
    pub fn account_is(&self, id: i64) {
        *self
            .script
            .account_id
            .lock()
            .expect("the stub's lock is not poisoned") = id;
    }

    /// Stops the stub.
    pub async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

async fn create_pin(
    State(script): State<Arc<PlexTvScript>>,
    headers: HeaderMap,
) -> (StatusCode, Json<serde_json::Value>) {
    // plex.tv echoes the identifier the caller sent unless the script says to
    // report a different one, which is the mismatch case.
    let sent = headers
        .get("x-plex-client-identifier")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let reported = script
        .client_identifier
        .lock()
        .expect("the stub's lock is not poisoned")
        .clone()
        .unwrap_or(sent);

    (
        StatusCode::CREATED,
        Json(json!({
            "id": 987_654,
            "code": "wxyz",
            "clientIdentifier": reported,
            "expiresIn": 900,
        })),
    )
}

async fn poll_pin(
    State(script): State<Arc<PlexTvScript>>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let token = script
        .authorized
        .load(Ordering::SeqCst)
        .then_some("plex-token-from-the-stub");
    (
        StatusCode::OK,
        Json(json!({ "id": id, "code": "wxyz", "authToken": token })),
    )
}

async fn account(
    State(script): State<Arc<PlexTvScript>>,
    headers: HeaderMap,
) -> (StatusCode, Json<serde_json::Value>) {
    if headers.get("x-plex-token").is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "no token" })),
        );
    }
    let id = *script
        .account_id
        .lock()
        .expect("the stub's lock is not poisoned");
    let username = script
        .username
        .lock()
        .expect("the stub's lock is not poisoned")
        .clone();
    (
        StatusCode::OK,
        Json(json!({ "id": id, "uuid": "stub-uuid", "username": username })),
    )
}
