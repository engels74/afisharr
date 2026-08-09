// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `afisharr start`.

use afisharr_core::settings::SettingsBody;
use anyhow::{Context, Result};
use tracing::info;

use crate::{configuration::DataPaths, startup};

/// Boots the instance and holds it open until it is asked to stop.
///
/// There is no HTTP surface yet — it arrives with the API crate — so the
/// process boots, reports that it is up, and waits. That is the honest shape of
/// the skeleton: everything before the listener already runs on every start.
///
/// # Errors
/// Returns whatever the boot sequence refused to start on.
pub async fn run(paths: &DataPaths, configured: SettingsBody) -> Result<()> {
    let booted = startup::boot(paths, configured).await?;

    info!("waiting for shutdown");
    tokio::signal::ctrl_c()
        .await
        .context("waiting for the shutdown signal")?;

    info!("shutting down");
    booted.database.close().await;
    Ok(())
}
