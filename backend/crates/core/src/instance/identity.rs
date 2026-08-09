// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The single `instance` row.

use sqlx::{SqliteConnection, SqlitePool};

use crate::{identifier::Id, storage::WriteOperation, time::Timestamp};

/// The identity of this installation.
//
// `instance_id` and `client_identifier` keep their prefixes: both are written
// into columns of those names, and shortening either here would put a rename in
// front of every reader comparing the struct to the schema.
#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    /// ULID of this installation.
    pub instance_id: String,
    /// `X-Plex-Client-Identifier`. Generated once, never regenerated.
    pub client_identifier: String,
    /// The name this instance presents to Plex.
    pub device_name: String,
    /// IANA timezone the engine's date operators are day-aligned in.
    pub timezone: String,
    /// Interface language tag.
    pub locale: String,
    /// The last binary that opened this database.
    pub app_version: String,
    /// When this installation first started.
    pub first_started_at: Timestamp,
    /// When the setup wizard finished, if it has.
    pub setup_completed_at: Option<Timestamp>,
    /// Wizard steps that complete by acknowledgement rather than by writing.
    pub setup_acked_steps: Vec<String>,
    /// When the row was last touched.
    pub updated_at: Timestamp,
}

/// The values a first start needs in order to mint an instance.
#[derive(Debug, Clone)]
pub struct NewInstance {
    /// The name this instance presents to Plex.
    pub device_name: String,
    /// IANA timezone.
    pub timezone: String,
    /// Interface language tag.
    pub locale: String,
    /// The version of the binary performing this start.
    pub app_version: String,
}

/// Reads the instance row, if this database has ever been started.
///
/// # Errors
/// Returns the underlying `sqlx` failure, or a decode failure if
/// `setup_acked_steps` does not hold a JSON array of step names.
pub async fn load(readers: &SqlitePool) -> Result<Option<Instance>, sqlx::Error> {
    sqlx::query_as!(Row, "SELECT * FROM instance WHERE id = 1")
        .fetch_optional(readers)
        .await?
        .map(Instance::try_from)
        .transpose()
}

/// Creates the instance row on first start, or refreshes what a restart changes.
///
/// `instance_id` and `client_identifier` are written once. A restart updates
/// `app_version`, `device_name`, `timezone`, `locale`, and `updated_at` and
/// leaves both identifiers alone — a regenerated `client_identifier` makes every
/// token plex.tv holds belong to a device the operator has never seen.
#[derive(Debug)]
pub struct EnsureInstance {
    /// What to write if the row does not exist yet.
    pub identity: NewInstance,
    /// The installation identifier minted for a first start.
    pub instance_id: Id,
    /// The Plex client identifier minted for a first start.
    pub client_identifier: Id,
    /// The instant of this start.
    pub at: Timestamp,
}

impl WriteOperation for EnsureInstance {
    type Output = Instance;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<Instance, sqlx::Error> {
        let instance_id = self.instance_id.as_str().to_owned();
        let client_identifier = self.client_identifier.as_str().to_owned();
        let NewInstance {
            device_name,
            timezone,
            locale,
            app_version,
        } = self.identity;
        let at = self.at.as_millis();

        sqlx::query!(
            "INSERT INTO instance (
                 id, instance_id, client_identifier, device_name, timezone, locale,
                 app_version, first_started_at, updated_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
             ON CONFLICT(id) DO UPDATE SET
                 device_name = excluded.device_name,
                 timezone    = excluded.timezone,
                 locale      = excluded.locale,
                 app_version = excluded.app_version,
                 updated_at  = excluded.updated_at",
            instance_id,
            client_identifier,
            device_name,
            timezone,
            locale,
            app_version,
            at
        )
        .execute(&mut *conn)
        .await?;

        Instance::try_from(
            sqlx::query_as!(Row, "SELECT * FROM instance WHERE id = 1")
                .fetch_one(&mut *conn)
                .await?,
        )
    }
}

/// The `instance` row exactly as `SQLite` returns it.
struct Row {
    id: i64,
    instance_id: String,
    client_identifier: String,
    device_name: String,
    timezone: String,
    locale: String,
    app_version: String,
    first_started_at: i64,
    setup_completed_at: Option<i64>,
    setup_acked_steps: String,
    updated_at: i64,
}

impl TryFrom<Row> for Instance {
    type Error = sqlx::Error;

    fn try_from(row: Row) -> Result<Self, Self::Error> {
        debug_assert_eq!(row.id, 1, "the instance table is constrained to one row");
        Ok(Self {
            instance_id: row.instance_id,
            client_identifier: row.client_identifier,
            device_name: row.device_name,
            timezone: row.timezone,
            locale: row.locale,
            app_version: row.app_version,
            first_started_at: Timestamp::from_millis(row.first_started_at),
            setup_completed_at: row.setup_completed_at.map(Timestamp::from_millis),
            // A body that is valid JSON but not a list of step names is a
            // corrupt row, not an empty acknowledgement list: reporting it as
            // empty would silently re-run wizard steps the operator finished.
            setup_acked_steps: serde_json::from_str(&row.setup_acked_steps)
                .map_err(|source| sqlx::Error::Decode(Box::new(source)))?,
            updated_at: Timestamp::from_millis(row.updated_at),
        })
    }
}
