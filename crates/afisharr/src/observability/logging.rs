// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The rotated application log.

use std::path::Path;

use anyhow::{Context, Result};
use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Keeps the log writer's worker thread alive for the process's lifetime.
///
/// Dropping it flushes and stops the writer, so it is held by `main` rather
/// than discarded at the end of initialisation — a dropped guard means the last
/// lines before a crash never reach the file, which is exactly the window a
/// support log exists for.
#[derive(Debug)]
pub struct LogGuard {
    // Held, never read: the writer thread stops when this drops, which is the
    // whole contract. `dead_code` sees a field nobody reads and is right about
    // the code and wrong about the purpose.
    #[allow(dead_code)]
    worker: WorkerGuard,
}

/// Starts logging to the console and to `logs/afisharr.log`, rotated daily.
///
/// This is the text log for support (PRD §19.2). It is not the run-event log
/// the interface reads: that is database-backed, scoped to a job run, and built
/// with the jobs surface. Two different readers, two different stores.
///
/// # Errors
/// Returns an error when the log directory cannot be created.
pub fn init(directory: &Path, filter: &str) -> Result<LogGuard> {
    std::fs::create_dir_all(directory)
        .with_context(|| format!("creating the log directory {}", directory.display()))?;

    let (writer, guard) = tracing_appender::non_blocking(rolling::daily(directory, "afisharr.log"));

    let filter = EnvFilter::try_new(filter)
        .or_else(|_| EnvFilter::try_new("info"))
        .with_context(|| format!("the log filter '{filter}' is not a tracing directive"))?;

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true))
        .with(fmt::layer().with_ansi(false).with_writer(writer))
        .init();

    Ok(LogGuard { worker: guard })
}
