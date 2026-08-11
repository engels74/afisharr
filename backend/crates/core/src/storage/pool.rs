// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The open database: a read pool beside one write actor.

use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Mutex,
    thread,
};

use sqlx::{ConnectOptions, SqlitePool, sqlite::SqlitePoolOptions};
use tokio::task::JoinHandle;

use crate::storage::{
    StorageError, WriteHandle,
    pragmas::{reader_options, writer_options},
    writer,
};

/// The ceiling PRD §19.4 puts on the read pool: `min(4, cores)`.
const MAX_READERS: usize = 4;

/// An open Afisharr database.
///
/// Holds the read pool and the handle to the single write actor. Cloning is
/// deliberately not offered: one process opens the database once, and the
/// handles inside are what get shared.
#[derive(Debug)]
pub struct Database {
    path: PathBuf,
    readers: SqlitePool,
    writer: WriteHandle,
    // Taken once by `close`. Behind a `Mutex<Option<_>>` because the database
    // is shared through an `Arc` — the HTTP surface holds one and so does the
    // boot sequence — and awaiting a `JoinHandle` needs to own it.
    writer_task: Mutex<Option<JoinHandle<()>>>,
}

impl Database {
    /// Opens `path`, creating the file and its directory if they do not exist.
    ///
    /// The write connection is established first, so the file exists — with the
    /// page size and auto-vacuum mode that can only be chosen at creation —
    /// before any reader connects.
    ///
    /// # Errors
    /// Returns [`StorageError::Directory`] if the parent directory cannot be
    /// created, or [`StorageError::Open`] if either connection fails.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|source| StorageError::Directory {
                    path: path.clone(),
                    source,
                })?;
        }

        let write_connection =
            writer_options(&path)
                .connect()
                .await
                .map_err(|source| StorageError::Open {
                    path: path.clone(),
                    source,
                })?;
        let (writer, writer_task) = writer::spawn(write_connection);

        let readers = SqlitePoolOptions::new()
            .max_connections(reader_count())
            .connect_with(reader_options(&path))
            .await
            .map_err(|source| StorageError::Open {
                path: path.clone(),
                source,
            })?;

        Ok(Self {
            path,
            readers,
            writer,
            writer_task: Mutex::new(Some(writer_task)),
        })
    }

    /// The read-only pool. Every query that does not mutate goes through this.
    #[must_use]
    pub fn readers(&self) -> &SqlitePool {
        &self.readers
    }

    /// The handle to the write actor. Every mutation goes through this.
    #[must_use]
    pub fn writer(&self) -> &WriteHandle {
        &self.writer
    }

    /// The database file's path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Closes both halves, waiting for the write actor to finish its queue.
    ///
    /// Takes `&self` and not `self`, because the database is shared through an
    /// `Arc` — the HTTP surface holds one and so does the boot sequence — and
    /// no holder can consume what the others still reference. That gives up a
    /// guarantee the old signature made for free: with `self`, "a query after
    /// close" was unrepresentable. Here it is a rule instead, and the rule is
    /// that the server drains first. `cli::start` is the one place that keeps
    /// it, and it keeps it as far as a shutdown can be kept: `serving.run`
    /// returns when the graceful drain finishes, or when the drain deadline
    /// elapses, and it returns only after nothing new can be accepted.
    ///
    /// The deadline is the case worth stating plainly, because it is the one
    /// the doc used to claim away. `docker stop` kills the container ten
    /// seconds after `SIGTERM`, so the drain is bounded — a response this
    /// instance cannot end from here must not cost the whole shutdown. When
    /// that bound is reached, a handler still running is still holding this
    /// database: its next read answers `PoolClosed` and its next write
    /// `WriterStopped`, so the operator's last request answers 500 rather than
    /// the listing or the session it was about to produce. `listener.rs` logs
    /// that it happened, naming the window, which is the only honest report
    /// available — the alternative is a shutdown the orchestrator kills instead.
    ///
    /// Take-once rather than merely harmless to repeat: the first caller takes
    /// the write actor's handle and does the work, and a second returns without
    /// closing a pool the first is still draining.
    pub async fn close(&self) {
        let task = self
            .writer_task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let Some(task) = task else {
            return;
        };
        self.readers.close().await;
        self.writer.shutdown().await;
        drop(task.await);
    }
}

/// `min(4, cores)`, and at least one when the host will not report a count.
fn reader_count() -> u32 {
    let cores = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    u32::try_from(cores.min(MAX_READERS)).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn the_read_pool_never_exceeds_four_connections() {
        assert!((1..=4).contains(&reader_count()));
    }

    #[tokio::test]
    async fn open_creates_the_file_with_the_one_way_door_pragmas() {
        let dir = TempDir::new().unwrap();
        let db = Database::open(dir.path().join("nested").join("afisharr.db"))
            .await
            .unwrap();

        let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
            .fetch_one(db.readers())
            .await
            .unwrap();
        let auto_vacuum: i64 = sqlx::query_scalar("PRAGMA auto_vacuum")
            .fetch_one(db.readers())
            .await
            .unwrap();
        let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(db.readers())
            .await
            .unwrap();

        assert_eq!(page_size, 8192);
        assert_eq!(auto_vacuum, 2, "2 is INCREMENTAL");
        assert_eq!(journal_mode, "wal");
        db.close().await;
    }

    #[tokio::test]
    async fn the_read_pool_refuses_to_write() {
        let dir = TempDir::new().unwrap();
        let db = Database::open(dir.path().join("afisharr.db"))
            .await
            .unwrap();

        let refusal = sqlx::query("CREATE TABLE smuggled (id TEXT)")
            .execute(db.readers())
            .await
            .expect_err("a read-only connection must refuse DDL");

        assert!(
            refusal.to_string().contains("readonly"),
            "expected a read-only refusal, got: {refusal}"
        );
        db.close().await;
    }
}
