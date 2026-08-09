// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Driving a plex.tv sign-in the way a browser does.

use reqwest::{Client, Response};

use crate::harness::{RunningInstance, csrf_from};

/// The header the CSRF cookie is echoed in.
pub const CSRF_HEADER: &str = "x-afisharr-csrf";

/// A client with a cookie jar, which is the only thing that makes it a browser.
pub fn browser() -> Client {
    Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("the test client must build")
}

/// One browser part-way through a plex.tv sign-in.
///
/// Starting an attempt hands the browser two cookies: the one that binds the
/// attempt to it, and the CSRF token that every completing call must echo. A
/// test that keeps neither is testing the refusal, not the exchange — which is
/// why they live here rather than in each test.
pub struct Attempt {
    pub client: Client,
    pub started: serde_json::Value,
    pub csrf: String,
}

impl Attempt {
    /// Starts a sign-in in a fresh browser.
    pub async fn start(running: &RunningInstance, oauth: bool) -> Self {
        Self::start_in(browser(), running, oauth).await
    }

    /// Starts a sign-in in `client`'s browser.
    pub async fn start_in(client: Client, running: &RunningInstance, oauth: bool) -> Self {
        let mut body = serde_json::json!({ "oauth": oauth });
        if oauth {
            // This instance's own address, which is what a browser sends and
            // the only target the endpoint forwards an operator back to.
            body["forwardUrl"] = serde_json::json!(format!("{}/login", running.base_url));
        }
        let response = client
            .post(format!("{}/api/auth/plex/pin", running.base_url))
            .json(&body)
            .send()
            .await
            .expect("the start route must answer");
        let csrf = csrf_from(&response).expect("starting must hand out a CSRF cookie");
        let started: serde_json::Value = response.json().await.expect("a JSON body");
        Self {
            client,
            started,
            csrf,
        }
    }

    /// This attempt's identifier.
    pub fn id(&self) -> &str {
        self.started["id"].as_str().expect("an attempt id")
    }

    /// Asks whether the exchange has finished, as the interface does.
    pub async fn poll(&self, running: &RunningInstance) -> Response {
        self.client
            .post(format!(
                "{}/api/auth/plex/pin/{}",
                running.base_url,
                self.id()
            ))
            .header(CSRF_HEADER, &self.csrf)
            .send()
            .await
            .expect("the poll route must answer")
    }

    /// One poll's body.
    pub async fn poll_body(&self, running: &RunningInstance) -> serde_json::Value {
        self.poll(running).await.json().await.expect("a JSON body")
    }
}
