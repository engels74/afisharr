// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Driving the first-run wizard the way a browser does.
//!
//! The setup claim is an ambient credential, so every state-changing wizard
//! call is judged by the CSRF layer and has to echo the token the claim handed
//! out. A test that posts without it is testing the CSRF refusal, not the
//! wizard — which is why the echo lives here rather than in each test.

use reqwest::{Client, Response, StatusCode};

use crate::harness::RunningInstance;

/// The header the CSRF cookie is echoed in.
const CSRF_HEADER: &str = "x-afisharr-csrf";

/// A browser holding the wizard.
pub struct Wizard {
    /// The client whose jar holds the claim and CSRF cookies.
    pub client: Client,
    /// The value every state-changing wizard call must echo.
    pub csrf: String,
}

impl Wizard {
    /// Claims the wizard with the token the console banner printed.
    pub async fn claim(running: &RunningInstance) -> Self {
        let token = running
            .token
            .clone()
            .expect("a fresh instance mints a token");
        let client = Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("the test client must build");

        let granted = client
            .post(format!("{}/api/setup/claim", running.base_url))
            .json(&serde_json::json!({ "token": token }))
            .send()
            .await
            .expect("the claim route must answer");
        assert_eq!(
            granted.status(),
            StatusCode::OK,
            "the claim must be granted"
        );

        let csrf = csrf_from(&granted).expect("claiming must hand out a CSRF cookie");
        Self { client, csrf }
    }

    /// Creates the first-run administrator.
    pub async fn create_admin(
        &self,
        running: &RunningInstance,
        username: &str,
        password: &str,
    ) -> Response {
        self.client
            .post(format!("{}/api/setup/admin", running.base_url))
            .header(CSRF_HEADER, &self.csrf)
            .json(&serde_json::json!({ "username": username, "password": password }))
            .send()
            .await
            .expect("the admin route must answer")
    }

    /// Finishes setup.
    pub async fn complete(&self, running: &RunningInstance) -> Response {
        self.client
            .post(format!("{}/api/setup/complete", running.base_url))
            .header(CSRF_HEADER, &self.csrf)
            .send()
            .await
            .expect("the complete route must answer")
    }

    /// Claims, creates the administrator, and finishes setup.
    ///
    /// What the interface does on a fresh instance, and what most tests need
    /// before they can begin.
    pub async fn set_up(running: &RunningInstance, username: &str, password: &str) -> Self {
        let wizard = Self::claim(running).await;
        let created = wizard.create_admin(running, username, password).await;
        assert_eq!(
            created.status(),
            StatusCode::OK,
            "the administrator must be created"
        );
        let completed = wizard.complete(running).await;
        assert_eq!(
            completed.status(),
            StatusCode::OK,
            "setup must be completable once an administrator exists"
        );
        wizard
    }
}

/// The CSRF cookie a response set, as the page would read it.
pub fn csrf_from(response: &Response) -> Option<String> {
    response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|cookie| cookie.strip_prefix("afisharr_csrf="))
        .and_then(|rest| rest.split(';').next())
        .map(str::to_owned)
}
