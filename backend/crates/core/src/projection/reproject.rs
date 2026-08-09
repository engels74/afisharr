// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The sweep behind `afisharr db reproject`.

use sqlx::{SqliteConnection, SqlitePool};

use crate::{
    projection::{
        DefinitionColumns, LifecycleAxes, ProjectionError, StateInputs, project_definition,
        project_state_hash,
    },
    storage::{WriteHandle, WriteOperation},
};

/// How many rows one read-compute-commit checkpoint covers.
///
/// The sweep's working set is a function of this number, never of the table's
/// size (`I-PERF-1`), and each batch commits in its own short transaction.
const BATCH: i64 = 500;

/// What a sweep found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Reprojection {
    /// Definition rows read.
    pub definitions_checked: u64,
    /// Definition rows whose stored columns disagreed with their body.
    pub definitions_corrected: u64,
    /// Item-state rows read.
    pub item_states_checked: u64,
    /// Item-state rows whose stored digest disagreed with its inputs.
    pub item_states_corrected: u64,
}

impl Reprojection {
    /// True when nothing needed correcting — what a healthy database reports.
    #[must_use]
    pub const fn is_noop(&self) -> bool {
        self.definitions_corrected == 0 && self.item_states_corrected == 0
    }
}

/// Recomputes every derived column from its body and writes back what drifted.
///
/// Reads stream in keyset batches and each batch commits on its own, so the
/// sweep is resumable and never holds the table in memory.
///
/// # Errors
/// Returns [`ProjectionError`] naming the row when a body cannot be projected,
/// and stops: a database with one corrupt envelope should be reported, not
/// half-swept.
#[tracing::instrument(skip_all)]
pub async fn reproject(
    readers: &SqlitePool,
    writer: &WriteHandle,
) -> Result<Reprojection, ProjectionError> {
    let mut report = Reprojection::default();
    reproject_definitions(readers, writer, &mut report).await?;
    reproject_item_states(readers, writer, &mut report).await?;
    tracing::info!(?report, "reprojection complete");
    Ok(report)
}

async fn reproject_definitions(
    readers: &SqlitePool,
    writer: &WriteHandle,
    report: &mut Reprojection,
) -> Result<(), ProjectionError> {
    let mut after = String::new();
    loop {
        let rows = sqlx::query!(
            "SELECT id, body_json, kind, handle, name, schema_version, registry_version,
                    body_hash, origin_kind, origin_pack, origin_pack_version
             FROM definitions WHERE id > ?1 ORDER BY id LIMIT ?2",
            after,
            BATCH
        )
        .fetch_all(readers)
        .await
        .map_err(|source| ProjectionError::Storage(source.into()))?;

        if rows.is_empty() {
            return Ok(());
        }

        let mut corrections = Vec::new();
        for row in rows {
            report.definitions_checked += 1;
            after.clone_from(&row.id);

            let projected = project_definition(&row.id, &row.body_json)?;
            let stored = DefinitionColumns {
                kind: row.kind,
                handle: row.handle,
                name: row.name,
                schema_version: row.schema_version,
                registry_version: row.registry_version,
                body_hash: row.body_hash,
                origin_kind: row.origin_kind,
                origin_pack: row.origin_pack,
                origin_pack_version: row.origin_pack_version,
            };
            if projected != stored {
                corrections.push((row.id, projected));
            }
        }

        report.definitions_corrected += corrections.len() as u64;
        if !corrections.is_empty() {
            writer
                .submit(WriteDefinitionColumns { corrections })
                .await?;
        }
    }
}

async fn reproject_item_states(
    readers: &SqlitePool,
    writer: &WriteHandle,
    report: &mut Reprojection,
) -> Result<(), ProjectionError> {
    let mut after = String::new();
    loop {
        // A show carries one subject per season plus one for the whole title.
        // The item's state is the whole title's, so the join takes the subject
        // with the lowest `season_number`, and `NULL` sorts first as -1.
        let rows = sqlx::query!(
            "SELECT s.library_item_id, s.facts_hash, s.ratings_hash, s.state_hash,
                    l.phase, l.acquisition, l.presence, l.production
             FROM library_item_state s
             LEFT JOIN lifecycle_subjects l
               ON l.id = (SELECT id FROM lifecycle_subjects
                          WHERE library_item_id = s.library_item_id
                          ORDER BY IFNULL(season_number, -1), id LIMIT 1)
             WHERE s.library_item_id > ?1
             ORDER BY s.library_item_id LIMIT ?2",
            after,
            BATCH
        )
        .fetch_all(readers)
        .await
        .map_err(|source| ProjectionError::Storage(source.into()))?;

        if rows.is_empty() {
            return Ok(());
        }

        let mut corrections = Vec::new();
        for row in rows {
            report.item_states_checked += 1;
            after.clone_from(&row.library_item_id);

            let lifecycle = match (&row.phase, &row.acquisition, &row.presence) {
                (Some(phase), Some(acquisition), Some(presence)) => Some(LifecycleAxes {
                    phase,
                    acquisition,
                    presence,
                    production: row.production.as_deref(),
                }),
                _ => None,
            };
            let projected = project_state_hash(&StateInputs {
                facts_hash: &row.facts_hash,
                ratings_hash: row.ratings_hash.as_deref(),
                lifecycle,
            });
            if projected != row.state_hash {
                corrections.push((row.library_item_id, projected));
            }
        }

        report.item_states_corrected += corrections.len() as u64;
        if !corrections.is_empty() {
            writer.submit(WriteStateHashes { corrections }).await?;
        }
    }
}

/// Writes back one batch of corrected `definitions` columns.
struct WriteDefinitionColumns {
    corrections: Vec<(String, DefinitionColumns)>,
}

impl WriteOperation for WriteDefinitionColumns {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let mut tx = sqlx::Connection::begin(conn).await?;
        for (id, columns) in self.corrections {
            sqlx::query!(
                "UPDATE definitions SET kind = ?2, handle = ?3, name = ?4, schema_version = ?5,
                        registry_version = ?6, body_hash = ?7, origin_kind = ?8,
                        origin_pack = ?9, origin_pack_version = ?10
                 WHERE id = ?1",
                id,
                columns.kind,
                columns.handle,
                columns.name,
                columns.schema_version,
                columns.registry_version,
                columns.body_hash,
                columns.origin_kind,
                columns.origin_pack,
                columns.origin_pack_version
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await
    }
}

/// Writes back one batch of corrected `library_item_state.state_hash` values.
struct WriteStateHashes {
    corrections: Vec<(String, String)>,
}

impl WriteOperation for WriteStateHashes {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let mut tx = sqlx::Connection::begin(conn).await?;
        for (library_item_id, state_hash) in self.corrections {
            sqlx::query!(
                "UPDATE library_item_state SET state_hash = ?2 WHERE library_item_id = ?1",
                library_item_id,
                state_hash
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await
    }
}
