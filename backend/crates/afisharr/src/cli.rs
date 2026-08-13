// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The command line.

mod db;
mod openapi;
mod start;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{configuration::DataPaths, observability};

/// Afisharr — Plex collections, posters, and overlay manager.
#[derive(Debug, Parser)]
#[command(name = "afisharr", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the instance.
    Start,
    /// Database maintenance.
    Db {
        #[command(subcommand)]
        command: db::DbCommand,
    },
    /// Print the `OpenAPI` document the `TypeScript` client is generated from.
    Openapi,
}

impl Cli {
    /// Runs the selected command, or `start` when none was given.
    ///
    /// # Errors
    /// Returns whatever the command failed with, with the context that names
    /// which step of the start it was.
    pub async fn run(self) -> Result<()> {
        // Before anything touches the filesystem: the document is a property
        // of this binary's annotations, and CI generates it with no data
        // directory and no database to open.
        if matches!(self.command, Some(Command::Openapi)) {
            return openapi::run();
        }

        let paths = DataPaths::from_env()?;
        let configured = crate::configuration::load(&paths.config_file())?;

        // Logging is initialised from the configured document before anything
        // can fail interestingly, so a failed start is a log line rather than a
        // bare process exit. That also puts `logging` outside the rule that the
        // stored settings row is the source of truth — the row lives in a
        // database this has not opened yet. `configuration::load` carries the
        // list of groups decided before the row can be read.
        let _log_guard = observability::init(&paths.logs(), &configured.effective.logging)?;

        match self.command.unwrap_or(Command::Start) {
            Command::Start => start::run(&paths, configured).await,
            Command::Db { command } => command.run(&paths, configured).await,
            // Answered above, before the data directory was resolved.
            Command::Openapi => openapi::run(),
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn the_command_line_is_internally_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn no_subcommand_means_start() {
        let cli = Cli::try_parse_from(["afisharr"]).expect("the bare invocation must parse");
        assert!(cli.command.is_none());
    }

    #[test]
    fn openapi_parses() {
        let cli = Cli::try_parse_from(["afisharr", "openapi"]).expect("openapi must parse");
        assert!(matches!(cli.command, Some(Command::Openapi)));
    }

    #[test]
    fn db_reproject_parses() {
        let cli =
            Cli::try_parse_from(["afisharr", "db", "reproject"]).expect("db reproject must parse");
        assert!(matches!(cli.command, Some(Command::Db { .. })));
    }
}
