// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `SQLite` spine: connection pragmas, the read pool, and the write actor.
//!
//! `SQLite` in WAL mode permits many concurrent readers and exactly one writer.
//! Afisharr does not discover that by catching `SQLITE_BUSY`; it makes a second
//! concurrent write impossible by construction (D-024, PRD §19.4). Readers get a
//! read-only pool, and every mutation is a [`WriteOperation`] message to the one
//! task that owns the write connection.

mod error;
mod pool;
mod pragmas;
mod writer;

pub use error::StorageError;
pub use pool::Database;
pub use writer::{WriteHandle, WriteOperation};
