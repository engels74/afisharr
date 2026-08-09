// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The open database: a read pool beside one write actor.

use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
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
    writer_task: JoinHandle<()>,
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
            writer_task,
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
    pub async fn close(self) {
        self.readers.close().await;
        self.writer.shutdown().await;
        drop(self.writer_task.await);
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
