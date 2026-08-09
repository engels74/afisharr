// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Running `foreign_key_check` and `integrity_check`.

use sqlx::{Row, SqlitePool};

/// What the two checks found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IntegrityReport {
    /// One entry per broken reference: `table.rowid -> parent`.
    pub broken_references: Vec<String>,
    /// Whatever `integrity_check` reported other than `ok`.
    pub structural_problems: Vec<String>,
}

impl IntegrityReport {
    /// True when both checks came back clean.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.broken_references.is_empty() && self.structural_problems.is_empty()
    }
}

/// Runs both checks and reports everything they found.
///
/// Both are reported together rather than short-circuiting on the first: an
/// operator diagnosing a damaged database wants the whole picture, and the
/// second check is cheap once the first has read the file.
///
/// # Errors
/// Returns the underlying `sqlx` failure. A pragma that cannot run at all is a
/// different condition from a pragma that reports damage, and the caller needs
/// to be able to tell them apart.
pub async fn verify(readers: &SqlitePool) -> Result<IntegrityReport, sqlx::Error> {
    let broken_references = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(readers)
        .await?
        .iter()
        .map(describe_broken_reference)
        .collect();

    let structural_problems: Vec<String> =
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_all(readers)
            .await?
            .into_iter()
            .filter(|line| line != "ok")
            .collect();

    Ok(IntegrityReport {
        broken_references,
        structural_problems,
    })
}

/// `foreign_key_check` returns `(table, rowid, parent, fkid)` untyped.
fn describe_broken_reference(row: &sqlx::sqlite::SqliteRow) -> String {
    let table: String = row.try_get(0).unwrap_or_default();
    let rowid: i64 = row.try_get(1).unwrap_or_default();
    let parent: String = row.try_get(2).unwrap_or_default();
    format!("{table} rowid {rowid} references missing {parent}")
}
