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
/// A bracketed IPv6 literal is accepted as well as a bare one. `[::]` is how
/// the wildcard is written everywhere an address and a port appear together —
/// in a URL, in a `docker run -p` argument, in nginx's `listen` — so it is what
/// an operator writes in `bindAddress`, and `ToSocketAddrs` parses it as
/// neither an address nor a resolvable name. The bind then failed on a value
/// that names exactly what the bare form names.
///
/// # Errors
/// Returns an error naming the address when the socket cannot be bound, which
/// on a container start almost always means the port is already taken.
pub async fn serve(address: &str, port: u16) -> Result<Serving> {
    let listener = TcpListener::bind((host_of(address), port))
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

/// The configured address as `ToSocketAddrs` will accept it.
///
/// Only the brackets come off. Everything else is passed through exactly as the
/// operator wrote it, so a value that does not name anything still fails at the
/// bind with the address in the message rather than being quietly reinterpreted
/// here.
fn host_of(address: &str) -> &str {
    address
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(address)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bracketed_ipv6_wildcard_names_the_same_thing_as_the_bare_one() {
        // `[::]` is how the wildcard is written wherever an address and a port
        // appear together, so it is what an operator puts in `bindAddress` —
        // and `ToSocketAddrs` reads it as neither an address nor a name, so the
        // bind failed on a value that names exactly what `::` names.
        assert_eq!(host_of("[::]"), "::");
        assert_eq!(host_of("[::1]"), "::1");
        assert_eq!(host_of("[fd00::1]"), "fd00::1");
    }

    #[test]
    fn everything_else_is_passed_through_as_written() {
        // Including a value that names nothing: it fails at the bind, with the
        // address the operator wrote in the message, rather than being turned
        // into some other address here.
        for address in ["0.0.0.0", "::", "192.168.1.10", "localhost", "[not-an-ip"] {
            assert_eq!(host_of(address), address);
        }
    }

    #[tokio::test]
    async fn a_bound_listener_reports_the_port_the_operating_system_chose() {
        // Port 0 is what the tests bind, and it is why `Serving` carries the
        // address at all: the configured document says 0 and the socket is
        // somewhere else.
        let serving = serve("127.0.0.1", 0).await.expect("the socket must bind");
        assert_ne!(serving.address.port(), 0);
        assert_eq!(serving.address.ip().to_string(), "127.0.0.1");
    }
}
