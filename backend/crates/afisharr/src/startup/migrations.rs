// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What this binary knows about the schema, and what the file actually holds.

use afisharr_core::storage::WriteOperation;
use anyhow::{Result, bail};
use sqlx::{
    SqliteConnection, SqlitePool,
    migrate::{MigrateError, Migrator},
};

/// Every migration compiled into this binary.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// What the database holds against what this binary carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationState {
    /// Versions recorded as applied, ascending.
    pub applied: Vec<i64>,
    /// Versions this binary carries that have not been applied, ascending.
    pub pending: Vec<i64>,
    /// The newest version this binary carries.
    pub newest_known: i64,
}

impl MigrationState {
    /// True when this database was written by a newer binary.
    #[must_use]
    pub fn is_newer_than_binary(&self) -> bool {
        self.applied
            .iter()
            .any(|applied| *applied > self.newest_known)
    }

    /// Refuses a database this binary must not open.
    ///
    /// A downgrade running against a newer schema corrupts data quietly, so
    /// refusing is the only safe answer. The message names both versions:
    /// "incompatible schema" alone tells the operator nothing about which
    /// binary to go and find.
    ///
    /// # Errors
    /// Returns an error naming the version found and the newest this binary
    /// carries.
    pub fn ensure_openable(&self) -> Result<()> {
        if let Some(found) = self.applied.iter().copied().max()
            && found > self.newest_known
        {
            bail!(
                "this database is at schema version {found}, which this binary does not know; \
                 the newest migration it carries is {}. Run the newer Afisharr, or restore a \
                 backup taken before the upgrade.",
                self.newest_known
            );
        }
        Ok(())
    }
}

/// Reads what the database holds and compares it with what this binary carries.
///
/// A database with no bookkeeping table has had no migration applied, which is
/// a first start rather than a failure.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn inspect(readers: &SqlitePool) -> Result<MigrationState, sqlx::Error> {
    let newest_known = MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or(0);

    let applied: Vec<i64> = if has_bookkeeping_table(readers).await? {
        sqlx::query_scalar!(
            "SELECT version FROM _sqlx_migrations WHERE success = TRUE ORDER BY version"
        )
        .fetch_all(readers)
        .await?
        .into_iter()
        // sqlx declares its own bookkeeping column as a nullable `BIGINT
        // PRIMARY KEY`; a row without a version is not a migration, so it is
        // dropped rather than counted as version zero.
        .flatten()
        .collect()
    } else {
        Vec::new()
    };

    let pending = MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .filter(|version| !applied.contains(version))
        .collect();

    Ok(MigrationState {
        applied,
        pending,
        newest_known,
    })
}

/// Applies every pending migration.
#[derive(Debug)]
pub struct RunMigrations;

impl WriteOperation for RunMigrations {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        // `run_direct` rather than `run`: sqlx provides it precisely because
        // `Migrator::run` cannot be called with a `&mut Connection` reborrowed
        // inside an async trait method — the `Acquire` bound is not general
        // enough over the two lifetimes RPITIT introduces. `false` is its
        // `skip` flag: `true` would record every migration as applied without
        // running a statement of it.
        MIGRATOR
            .run_direct(None, conn, false)
            .await
            .map_err(|error| match error {
                MigrateError::Execute(source) => source,
                other => sqlx::Error::Migrate(Box::new(other)),
            })
    }
}

async fn has_bookkeeping_table(readers: &SqlitePool) -> Result<bool, sqlx::Error> {
    let found: Option<String> = sqlx::query_scalar!(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'"
    )
    .fetch_optional(readers)
    .await?
    .flatten();
    Ok(found.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_database_at_a_known_version_opens() {
        let state = MigrationState {
            applied: vec![1],
            pending: vec![],
            newest_known: 1,
        };
        assert!(!state.is_newer_than_binary());
        assert!(state.ensure_openable().is_ok());
    }

    #[test]
    fn a_database_at_an_unknown_version_is_refused_naming_both_versions() {
        let state = MigrationState {
            applied: vec![1, 2],
            pending: vec![],
            newest_known: 1,
        };
        assert!(state.is_newer_than_binary());

        let message = format!(
            "{:#}",
            state
                .ensure_openable()
                .expect_err("a newer schema must be refused")
        );
        assert!(
            message.contains('2'),
            "the found version must be named: {message}"
        );
        assert!(
            message.contains('1'),
            "the binary's newest must be named: {message}"
        );
    }

    #[test]
    fn a_first_start_has_no_applied_versions_and_is_openable() {
        let state = MigrationState {
            applied: vec![],
            pending: vec![1],
            newest_known: 1,
        };
        assert!(state.ensure_openable().is_ok());
    }

    #[test]
    fn the_binary_carries_at_least_the_initial_schema() {
        assert!(MIGRATOR.iter().any(|migration| migration.version == 1));
    }
}
