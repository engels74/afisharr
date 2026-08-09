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
/// be created, [`BackupError::Copy`] when `SQLite` refuses either database or
/// the copy fails part-way, [`BackupError::Incomplete`] when the copy stops with
/// the source unread, and [`BackupError::TaskFailed`] when the blocking task
/// does not finish.
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
    let copied = tokio::task::spawn_blocking({
        let (from, to) = (from.clone(), to.clone());
        move || copy_blocking(&from, &to)
    })
    .await
    .map_err(|_| BackupError::TaskFailed)?;

    if copied.is_err() {
        // The destination was created before the copy failed, and retention
        // reads a name rather than a database: left there, a truncated file
        // ranks as the newest copy for its schema and is offered to an operator
        // as something to restore. A backup that is not real must not look like
        // one. Removal runs here so the connections are already dropped.
        if let Err(error) = tokio::fs::remove_file(&to).await
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(path = %to.display(), %error, "the failed copy could not be removed");
        }
    }

    copied
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
    require_done(step, from, to)?;

    Ok(to.to_path_buf())
}

/// Turns a step that copied less than the whole database into a failure.
///
/// `Backup::step` reports `Busy` and `Locked` as `Ok`: the call succeeded, the
/// copy did not. Left as a `debug_assert!`, a release build accepted them and
/// reported a destination that exists, is the wrong size, and may hold nothing
/// — the one outcome the online backup API is used here to rule out.
fn require_done(step: StepResult, from: &Path, to: &Path) -> Result<(), BackupError> {
    if matches!(step, StepResult::Done) {
        return Ok(());
    }
    Err(BackupError::Incomplete {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        step: match step {
            StepResult::More => "pages still outstanding",
            StepResult::Busy => "the source busy",
            StepResult::Locked => "the source locked",
            _ => "an unrecognised result",
        },
    })
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

    #[tokio::test]
    async fn a_failed_copy_leaves_nothing_behind_that_looks_like_a_backup() {
        let dir = TempDir::new().unwrap();
        let source = dir.path().join("afisharr.db");
        // Opening this succeeds; reading it as a database does not, so the
        // destination is created and then the copy fails.
        std::fs::write(&source, b"not a database").unwrap();
        let destination = dir.path().join("backups").join("pre-migration-1-1.db");

        let error = copy(&source, &destination)
            .await
            .expect_err("a source that is not a database cannot be backed up");

        assert!(matches!(error, BackupError::Copy { .. }), "{error:?}");
        assert!(
            !destination.exists(),
            "retention reads a name, so a truncated file would rank as the newest copy"
        );
    }

    #[test]
    fn only_a_finished_step_is_a_backup() {
        let from = Path::new("afisharr.db");
        let to = Path::new("copy.db");

        require_done(StepResult::Done, from, to).expect("a whole copy is a backup");

        for unfinished in [StepResult::More, StepResult::Busy, StepResult::Locked] {
            let error = require_done(unfinished, from, to)
                .expect_err("a copy that stopped early is not a backup");
            assert!(
                matches!(error, BackupError::Incomplete { .. }),
                "{unfinished:?} must be reported, not accepted: {error:?}"
            );
        }
    }
}
