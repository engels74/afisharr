// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Copying a live database, and keeping the last few copies.
//!
//! Migrations are forward-only, so *restore the pre-migration backup* is the
//! only honest recovery path from a bad upgrade (D-023). That makes the backup
//! a correctness feature: it is taken through `SQLite`'s online backup API, never
//! by copying the file, because a file copy of a WAL database mid-write is not
//! a valid database and the failure is silent — the copy exists, has the right
//! size, and opens (PRD §19.3, §21.6.2).

mod error;
mod online;
mod retention;

pub use error::BackupError;
pub use online::copy;
pub use retention::{PRE_MIGRATION_PREFIX, pre_migration_path, prune};
