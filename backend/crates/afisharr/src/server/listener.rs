// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Binding the socket and serving until asked to stop.

use std::{net::SocketAddr, time::Duration};

use afisharr_api::{router, state::ApiState};
use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing::{info, warn};

/// How long a response already in flight has to finish after the stop signal.
///
/// `docker stop` sends `SIGTERM` and kills the container ten seconds later, so
/// a drain that outlasts that is a drain nobody ever sees the end of. Five
/// seconds is enough for an ordinary answer to be written, and leaves the rest
/// of the budget for closing the database.
const DRAIN_GRACE: Duration = Duration::from_secs(5);

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
    /// Serves `state`'s router until `shutdown` completes, then drains.
    ///
    /// `into_make_service_with_connect_info` and not `into_make_service`: the
    /// peer address is what every rate limit is keyed on when no proxy is
    /// trusted, and a router built without it has no peer to fall back to
    /// (`I-SEC-1`).
    ///
    /// Two things happen on the signal, and the shutdown is not graceful
    /// without both. The event streams are closed, because a graceful stop
    /// waits for the responses already in flight and an SSE body never ends on
    /// its own — one open tab would otherwise hold the process until the
    /// container killed it. And the wait for whatever is left is bounded, so a
    /// response this instance cannot end from here — a client that has stopped
    /// reading, a body added later — costs [`DRAIN_GRACE`] rather than the
    /// whole shutdown.
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

        // Cloned before the state is consumed by the router: this is the handle
        // the signal ends the open streams through.
        let stream = state.stream().clone();
        let (draining, drain_started) = tokio::sync::oneshot::channel();
        let signal = async move {
            shutdown.await;
            stream.close();
            let _ = draining.send(());
        };

        let served = axum::serve(
            self.listener,
            router::build(state).into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(signal);

        tokio::select! {
            result = served => result.context("serving HTTP"),
            () = deadline(drain_started) => {
                warn!(
                    seconds = DRAIN_GRACE.as_secs(),
                    "a response did not finish inside the drain window; closing anyway"
                );
                Ok(())
            }
        }
    }
}

/// Completes [`DRAIN_GRACE`] after the stop signal, and never before it.
///
/// A deadline armed at startup would be a request timeout, which is not what
/// this is: nothing is hurried while the instance is serving, and the clock
/// starts only once the process has been asked to stop.
async fn deadline(started: tokio::sync::oneshot::Receiver<()>) {
    if started.await.is_err() {
        // The signal future was dropped, which happens when the server has
        // already finished. There is nothing left to bound.
        std::future::pending::<()>().await;
    }
    tokio::time::sleep(DRAIN_GRACE).await;
}
