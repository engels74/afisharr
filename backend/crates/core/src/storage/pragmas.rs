// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The pragmas every connection carries, and the two that create the file.

use std::{path::Path, time::Duration};

use sqlx::sqlite::{SqliteAutoVacuum, SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};

/// 32 MB of page cache per connection, expressed the way `SQLite` wants it:
/// a negative `cache_size` is a byte budget in kibibytes.
const CACHE_SIZE_KIB: &str = "-32000";

/// Options for the single write connection.
///
/// `page_size` and `auto_vacuum` are one-way doors (PRD §19.3): they take effect
/// only on the file's first write, and `sqlx migrate` writes its own bookkeeping
/// table before migration `0001` runs. Setting them here is what makes the
/// values in that migration true.
pub(crate) fn writer_options(path: &Path) -> SqliteConnectOptions {
    shared(SqliteConnectOptions::new().filename(path))
        .create_if_missing(true)
        .page_size(8192)
        .auto_vacuum(SqliteAutoVacuum::Incremental)
        .journal_mode(SqliteJournalMode::Wal)
}

/// Options for a pooled read connection.
///
/// Read-only is the structural half of D-024: a connection that cannot write is
/// not a second write path that review has to keep noticing.
pub(crate) fn reader_options(path: &Path) -> SqliteConnectOptions {
    shared(SqliteConnectOptions::new().filename(path))
        .create_if_missing(false)
        .read_only(true)
}

/// The pragmas PRD §19.4 sets on every pooled connection at acquisition.
fn shared(options: SqliteConnectOptions) -> SqliteConnectOptions {
    options
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5))
        .synchronous(SqliteSynchronous::Normal)
        .pragma("cache_size", CACHE_SIZE_KIB)
        .pragma("temp_store", "MEMORY")
}
