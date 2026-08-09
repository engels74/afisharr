// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The single `settings` row and its history.

use sqlx::{Connection, SqliteConnection, SqlitePool};

use crate::{
    settings::{SettingsBody, SettingsError},
    storage::WriteOperation,
    time::Timestamp,
};

/// The stored settings document and the version it was written as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Monotonic version, incremented once per accepted save.
    pub version: i64,
    /// The document.
    pub body: SettingsBody,
    /// When it was written.
    pub updated_at: Timestamp,
    /// Who wrote it, when a user did.
    pub updated_by: Option<String>,
}

/// Reads the settings document, if one has ever been written.
///
/// # Errors
/// Returns [`SettingsError::Malformed`] when the stored body no longer matches
/// the typed document, and [`SettingsError::Storage`] when the read fails.
pub async fn load(readers: &SqlitePool) -> Result<Option<Settings>, SettingsError> {
    let row = sqlx::query!("SELECT version, body_json, updated_at, updated_by FROM settings")
        .fetch_optional(readers)
        .await
        .map_err(|source| SettingsError::Storage(source.into()))?;

    row.map(|row| {
        Ok(Settings {
            version: row.version,
            body: serde_json::from_str(&row.body_json).map_err(SettingsError::Malformed)?,
            updated_at: Timestamp::from_millis(row.updated_at),
            updated_by: row.updated_by,
        })
    })
    .transpose()
}

/// Writes the whole settings document as one new version.
///
/// The row and its history entry are written in one transaction, so a version
/// that exists in `settings` always exists in `settings_history` too. There is
/// no partial write because there is no per-key write path.
#[derive(Debug)]
pub struct SaveSettings {
    /// The document to store.
    pub body: SettingsBody,
    /// Who is saving it, when a user is.
    pub actor: Option<String>,
    /// The instant of the save.
    pub at: Timestamp,
}

impl WriteOperation for SaveSettings {
    type Output = Settings;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<Settings, sqlx::Error> {
        let body_json =
            serde_json::to_string(&self.body).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
        let at = self.at.as_millis();
        let actor = self.actor;

        let mut tx = conn.begin().await?;

        let previous = sqlx::query!("SELECT version, body_json FROM settings")
            .fetch_optional(&mut *tx)
            .await?;
        let version = previous.as_ref().map_or(1, |row| row.version + 1);
        let diff_json = previous
            .map(|row| diff(&row.body_json, &body_json))
            .transpose()
            .map_err(|e| sqlx::Error::Encode(Box::new(e)))?;

        sqlx::query!(
            "INSERT INTO settings (id, version, body_json, updated_at, updated_by)
             VALUES (1, ?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                 version = excluded.version, body_json = excluded.body_json,
                 updated_at = excluded.updated_at, updated_by = excluded.updated_by",
            version,
            body_json,
            at,
            actor
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "INSERT INTO settings_history (version, body_json, changed_at, changed_by, diff_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            version,
            body_json,
            at,
            actor,
            diff_json
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(Settings {
            version,
            body: self.body,
            updated_at: self.at,
            updated_by: actor,
        })
    }
}

/// The set of top-level groups that changed between two stored bodies.
///
/// Deliberately coarse: the history exists so an operator can see *that* the
/// HTTP settings moved between two versions and read both bodies, not so a
/// diff algorithm becomes a second representation of the document.
fn diff(before: &str, after: &str) -> Result<String, serde_json::Error> {
    let before: serde_json::Value = serde_json::from_str(before)?;
    let after: serde_json::Value = serde_json::from_str(after)?;
    let changed: Vec<&str> = after
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(key, value)| before.get(key.as_str()) != Some(*value))
        .map(|(key, _)| key.as_str())
        .collect();
    serde_json::to_string(&serde_json::json!({ "changedGroups": changed }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn the_diff_names_only_the_groups_that_moved() {
        let before = serde_json::to_string(&SettingsBody::default()).unwrap();
        let mut changed = SettingsBody::default();
        changed.http.port = 9000;
        let after = serde_json::to_string(&changed).unwrap();

        let diff: serde_json::Value =
            serde_json::from_str(&diff(&before, &after).unwrap()).unwrap();
        assert_eq!(diff["changedGroups"], serde_json::json!(["http"]));
    }

    #[test]
    fn an_unchanged_body_produces_an_empty_diff() {
        let body = serde_json::to_string(&SettingsBody::default()).unwrap();
        let diff: serde_json::Value = serde_json::from_str(&diff(&body, &body).unwrap()).unwrap();
        assert_eq!(diff["changedGroups"], serde_json::json!([]));
    }
}
