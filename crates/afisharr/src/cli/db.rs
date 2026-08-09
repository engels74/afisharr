// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `afisharr db`.

use afisharr_core::{integrity, projection, settings::SettingsBody};
use anyhow::{Context, Result, bail};
use clap::Subcommand;
use tracing::info;

use crate::{configuration::DataPaths, startup};

/// Database maintenance commands.
#[derive(Debug, Subcommand)]
pub enum DbCommand {
    /// Recompute every derived column from its canonical body.
    Reproject,
    /// Run `foreign_key_check` and `integrity_check` and report what they find.
    Check,
}

impl DbCommand {
    /// Runs the command against a booted instance.
    ///
    /// # Errors
    /// Returns an error when the boot sequence refuses, when a body cannot be
    /// projected, or when the integrity checks report damage.
    pub async fn run(self, paths: &DataPaths, configured: SettingsBody) -> Result<()> {
        let booted = startup::boot(paths, configured).await?;
        let outcome = match self {
            Self::Reproject => reproject(&booted).await,
            Self::Check => check(&booted).await,
        };
        booted.database.close().await;
        outcome
    }
}

async fn reproject(booted: &startup::Booted) -> Result<()> {
    let report = projection::reproject(booted.database.readers(), booted.database.writer())
        .await
        .context("reprojecting derived columns")?;

    info!(
        definitions_checked = report.definitions_checked,
        definitions_corrected = report.definitions_corrected,
        item_states_checked = report.item_states_checked,
        item_states_corrected = report.item_states_corrected,
        "reprojection complete"
    );

    // The report goes to stdout as well as to the log: this command is run by a
    // person at a terminal, and a no-op is the answer they are looking for.
    println!(
        "definitions: {} checked, {} corrected\nitem states: {} checked, {} corrected\n{}",
        report.definitions_checked,
        report.definitions_corrected,
        report.item_states_checked,
        report.item_states_corrected,
        if report.is_noop() {
            "no drift found"
        } else {
            "drift corrected"
        }
    );
    Ok(())
}

async fn check(booted: &startup::Booted) -> Result<()> {
    let report = integrity::verify(booted.database.readers())
        .await
        .context("running the integrity checks")?;

    if report.is_clean() {
        println!("foreign_key_check: ok\nintegrity_check: ok");
        return Ok(());
    }

    bail!(
        "broken references: {:?}\nstructural problems: {:?}",
        report.broken_references,
        report.structural_problems
    );
}
