// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Binding the socket and serving until asked to stop.

use std::net::SocketAddr;

use afisharr_api::{router, state::ApiState};
use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing::info;

/// A bound listener and the address it actually got.
///
/// The address is reported rather than assumed: a test binds port 0 and needs
/// to know what the operating system chose.
#[derive(Debug)]
pub struct Serving {
    /// The address the socket is bound to.
    pub address: SocketAddr,
    listener: TcpListener,
}

/// Binds `address` and returns a listener ready to serve.
///
/// # Errors
/// Returns an error naming the address when the socket cannot be bound, which
/// on a container start almost always means the port is already taken.
pub async fn serve(address: &str, port: u16) -> Result<Serving> {
    let listener = TcpListener::bind((address, port))
        .await
        .with_context(|| format!("binding {address}:{port}"))?;
    let bound = listener
        .local_addr()
        .context("reading the bound socket's address")?;
    Ok(Serving {
        address: bound,
        listener,
    })
}

impl Serving {
    /// Serves `state`'s router until `shutdown` completes.
    ///
    /// `into_make_service_with_connect_info` and not `into_make_service`: the
    /// peer address is what every rate limit is keyed on when no proxy is
    /// trusted, and a router built without it has no peer to fall back to
    /// (`I-SEC-1`).
    ///
    /// # Errors
    /// Returns an error when the server stops for a reason other than the
    /// shutdown signal.
    pub async fn run(
        self,
        state: ApiState,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<()> {
        info!(address = %self.address, "serving HTTP");
        axum::serve(
            self.listener,
            router::build(state).into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown)
        .await
        .context("serving HTTP")
    }
}
