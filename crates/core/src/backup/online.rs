// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The copy itself, through `SQLite`'s online backup API.

use std::path::{Path, PathBuf};

use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
};

use crate::backup::BackupError;

/// Copies the database at `from` to `to` using `SQLite`'s online backup API.
///
/// The whole copy runs in one step so the destination is a consistent snapshot
/// rather than a moving target. It is blocking work and therefore runs on a
/// blocking thread rather than on a runtime worker.
///
/// # Errors
/// Returns [`BackupError::Directory`] when the destination's directory cannot
/// be created, and [`BackupError::Copy`] when `SQLite` refuses either database or
/// the copy fails part-way.
#[tracing::instrument(skip_all)]
pub async fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<PathBuf, BackupError> {
    let from = from.as_ref().to_path_buf();
    let to = to.as_ref().to_path_buf();

    if let Some(parent) = to.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|source| BackupError::Directory {
                path: parent.to_path_buf(),
                source,
            })?;
    }

    tracing::info!(from = %from.display(), to = %to.display(), "copying the database");
    tokio::task::spawn_blocking(move || copy_blocking(&from, &to))
        .await
        .map_err(|_| BackupError::TaskFailed)?
}

/// The blocking half, kept separate so the async wrapper reads as wiring.
fn copy_blocking(from: &Path, to: &Path) -> Result<PathBuf, BackupError> {
    let fail = |source| BackupError::Copy {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source,
    };

    let source =
        Connection::open_with_flags(from, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(fail)?;
    let mut destination = Connection::open(to).map_err(fail)?;

    // `-1` copies every remaining page in one step, and there is no progress
    // callback: a pre-migration backup blocks startup, so there is nobody to
    // report progress to and nothing to interleave with.
    let step = Backup::new(&source, &mut destination)
        .map_err(fail)?
        .step(-1)
        .map_err(fail)?;
    debug_assert!(
        matches!(step, StepResult::Done),
        "a single step of -1 pages copies the whole database"
    );

    Ok(to.to_path_buf())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn the_copy_carries_the_source_rows() {
        let dir = TempDir::new().unwrap();
        let source = dir.path().join("afisharr.db");
        {
            let conn = Connection::open(&source).unwrap();
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 CREATE TABLE t (id TEXT PRIMARY KEY) STRICT;
                 INSERT INTO t VALUES ('kept');",
            )
            .unwrap();
        }

        let destination = dir.path().join("backups").join("copy.db");
        let written = copy(&source, &destination).await.unwrap();

        assert_eq!(written, destination);
        let conn = Connection::open(&destination).unwrap();
        let value: String = conn
            .query_row("SELECT id FROM t", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "kept");
    }

    #[tokio::test]
    async fn a_missing_source_is_reported_rather_than_producing_an_empty_copy() {
        let dir = TempDir::new().unwrap();
        let error = copy(dir.path().join("absent.db"), dir.path().join("copy.db"))
            .await
            .expect_err("a database that does not exist cannot be backed up");
        assert!(matches!(error, BackupError::Copy { .. }));
    }
}
