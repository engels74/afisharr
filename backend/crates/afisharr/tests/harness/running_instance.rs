// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A booted instance with its HTTP surface actually listening.
//!
//! The tests that matter here are about what a request gets back, so they drive
//! a real socket rather than calling handlers directly: middleware ordering,
//! the peer address the limiter keys on, and `Set-Cookie` round-tripping are
//! all properties of the stack and not of any handler.

use std::sync::Arc;

use afisharr::{server, startup};
use afisharr_core::{settings::SettingsBody, setup::TokenStore, time::SystemClock};

use crate::harness::TempInstance;

/// A listening instance, with the token its console banner would have printed.
pub struct RunningInstance {
    /// Where the API is listening.
    pub base_url: String,
    /// The bootstrap token this instance minted, or `None` once setup is done.
    pub token: Option<String>,
    /// The booted instance, for tests that read the database directly.
    pub booted: startup::Booted,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl RunningInstance {
    /// Boots `instance` and serves it on a port the operating system chooses.
    pub async fn start(instance: &TempInstance) -> Self {
        Self::start_with(instance, SettingsBody::default()).await
    }

    /// The same, with a settings document of the test's own.
    pub async fn start_with(instance: &TempInstance, configured: SettingsBody) -> Self {
        Self::start_full(instance, configured, None).await
    }

    /// The same, pointed at a stand-in for plex.tv.
    pub async fn start_against_plex(instance: &TempInstance, plex_base: &str) -> Self {
        Self::start_full(instance, SettingsBody::default(), Some(plex_base)).await
    }

    /// The same, with nothing configured as the public origin.
    ///
    /// What an operator who never set one has. The hosted plex.tv sign-in is
    /// the one flow that needs an absolute address for this instance, so it is
    /// the one flow that has to say so rather than believing whatever authority
    /// the request carried.
    pub async fn start_without_public_origin(instance: &TempInstance, plex_base: &str) -> Self {
        Self::listening(instance, SettingsBody::default(), Some(plex_base), false).await
    }

    async fn start_full(
        instance: &TempInstance,
        configured: SettingsBody,
        plex_base: Option<&str>,
    ) -> Self {
        Self::listening(instance, configured, plex_base, true).await
    }

    async fn listening(
        instance: &TempInstance,
        configured: SettingsBody,
        plex_base: Option<&str>,
        with_public_origin: bool,
    ) -> Self {
        // The socket first, because its address is what a deployed instance has
        // configured as the origin operators reach it at — and here the port is
        // the operating system's to choose.
        let serving = server::serve("127.0.0.1", 0)
            .await
            .expect("binding an ephemeral port must succeed");
        let base_url = format!("http://{}", serving.address);

        let mut configured = configured;
        if with_public_origin && configured.http.public_origin.is_none() {
            configured.http.public_origin = Some(base_url.clone());
        }
        let booted = instance.boot_with(configured).await;

        let bootstrap = Arc::new(TokenStore::empty());
        let token = booted
            .instance
            .setup_completed_at
            .is_none()
            .then(|| bootstrap.mint(&SystemClock));

        let state = server::build_state_against(&booted, Arc::clone(&bootstrap), plex_base)
            .await
            .expect("the router's state must assemble");

        let (shutdown, stop) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = serving
                .run(state, async {
                    let _ = stop.await;
                })
                .await;
        });

        Self {
            base_url,
            token,
            booted,
            shutdown: Some(shutdown),
            task: Some(task),
        }
    }

    /// Stops the server and waits for it.
    pub async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
        self.booted.database.close().await;
    }
}
