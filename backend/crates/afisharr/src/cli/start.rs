// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `afisharr start`.

use std::sync::Arc;

use afisharr_core::{settings::SettingsBody, setup::TokenStore, time::SystemClock};
use anyhow::Result;
use tracing::{info, warn};

use crate::{bootstrap::print_setup_banner, configuration::DataPaths, interface, server, startup};

/// Boots the instance, serves it, and holds it open until it is asked to stop.
///
/// # Errors
/// Returns whatever the boot sequence refused to start on, or the failure that
/// stopped the server.
pub async fn run(paths: &DataPaths, configured: SettingsBody) -> Result<()> {
    let booted = startup::boot(paths, configured).await?;

    // Everything after the database is open runs inside `serve`, and its
    // outcome is held rather than propagated, because the close below has to
    // happen on the failing paths too. Three of the steps in there are fallible
    // — the state the configuration is judged in, the listener, and the server
    // itself — and the port being held by the container that has not finished
    // stopping is the failure `serve` names as almost always the cause. Each of
    // those returned before the close, so the read pool was dropped and the
    // write actor's queued commands went with it, leaving the WAL
    // uncheckpointed for the next start to replay.
    let outcome = serve_until_stopped(&booted).await;

    info!("shutting down");
    booted.database.close().await;
    outcome
}

/// Serves until the process is asked to stop.
///
/// Split from [`run`] so that every way out of it — including the ones that
/// never reach the server — passes through one close.
async fn serve_until_stopped(booted: &startup::Booted) -> Result<()> {
    // The token exists only while setup is incomplete, and minting it here —
    // once, on the start that prints it — is what makes a restart invalidate
    // the previous one (PRD §19.6.1).
    let bootstrap = Arc::new(TokenStore::empty());
    if booted.instance.setup_completed_at.is_none() {
        let token = bootstrap.mint(&SystemClock);
        print_setup_banner(&token, &booted.settings.body.http);
    }

    if !interface::EmbeddedInterface::is_present() {
        warn!(
            "this build carries no interface: the API is serving, but every page \
             answers that the SPA was not built into it"
        );
    }

    let state = server::build_state(booted, Arc::clone(&bootstrap)).await?;
    let http = &booted.settings.body.http;
    let serving = server::serve(&http.bind_address, http.port).await?;
    info!(address = %serving.address, "afisharr is listening");

    serving.run(state, shutdown_signal()).await
}

/// Completes when the process is asked to stop.
///
/// Both signals, not just Ctrl-C: `SIGTERM` is what `docker stop` sends, and a
/// container that only listens for `SIGINT` is a container that is killed after
/// its grace period every single time.
async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            // A process that cannot install the handler still stops on Ctrl-C,
            // and refusing to serve over it would be worse than the lost
            // graceful shutdown.
            Err(error) => {
                warn!(%error, "could not listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => info!("interrupted"),
        () = terminate => info!("terminated"),
    }
}
