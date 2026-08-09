// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Rows the schema tests need, written the way the application writes rows.

use afisharr_core::storage::{WriteHandle, WriteOperation};
use sqlx::SqliteConnection;

/// Inserts a library and returns its identifier.
///
/// Placement, lifecycle, and visibility all hang off a library, so every schema
/// test that reaches those tables starts here.
pub async fn seed_library(writer: &WriteHandle, id: &str) -> String {
    writer
        .submit(InsertLibrary { id: id.to_owned() })
        .await
        .expect("the library must insert");
    id.to_owned()
}

struct InsertLibrary {
    id: String,
}

impl WriteOperation for InsertLibrary {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let id = self.id;
        sqlx::query!(
            "INSERT INTO libraries
                 (id, handle, section_key, type, title, created_at, last_seen_at)
             VALUES (?1, ?1, '1', 'movie', 'Films', 0, 0)",
            id
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

/// Inserts a `PlexUser` principal — the per-user targeting `I-DATA-5` asserts.
pub struct InsertPlexPrincipal {
    /// The principal's ULID.
    pub id: String,
    /// The plex.tv numeric account id.
    pub plex_account_id: i64,
}

impl WriteOperation for InsertPlexPrincipal {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let Self {
            id,
            plex_account_id,
        } = self;
        sqlx::query!(
            "INSERT INTO principals (id, kind, plex_account_id, label, created_at)
             VALUES (?1, 'PlexUser', ?2, 'A household member', 0)",
            id,
            plex_account_id
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

/// Makes one participant visible to one principal on one surface.
pub struct InsertVisibility {
    /// The participant being targeted.
    pub participant_id: String,
    /// The library the participant belongs to.
    pub library_id: String,
    /// The principal that may see it.
    pub principal_id: String,
}

impl WriteOperation for InsertVisibility {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let Self {
            participant_id,
            library_id,
            principal_id,
        } = self;
        sqlx::query!(
            "INSERT INTO placement_participants
                 (id, type, library_id, hub_identifier, title, is_deletable,
                  first_seen_at, last_seen_at)
             VALUES (?1, 'NativeHub', ?2, 'home.continue', 'Continue Watching', 0, 0, 0)",
            participant_id,
            library_id
        )
        .execute(&mut *conn)
        .await?;

        sqlx::query!(
            "INSERT INTO placement_visibility (participant_id, surface, principal_id)
             VALUES (?1, 'Home', ?2)",
            participant_id,
            principal_id
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

/// Inserts a whole-title lifecycle subject.
pub struct InsertLifecycleSubject {
    /// The subject's ULID.
    pub id: String,
    /// The library it belongs to.
    pub library_id: String,
    /// The identifier space of its primary identity.
    pub id_space: String,
    /// The identifier value.
    pub id_value: String,
    /// `None` for a whole title, `Some` for one season.
    pub season_number: Option<i64>,
}

impl WriteOperation for InsertLifecycleSubject {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let Self {
            id,
            library_id,
            id_space,
            id_value,
            season_number,
        } = self;

        sqlx::query!(
            "INSERT OR IGNORE INTO lifecycle_policies (version, body_json, created_at)
                      VALUES (1, '{}', 0)"
        )
        .execute(&mut *conn)
        .await?;

        sqlx::query!(
            "INSERT INTO lifecycle_subjects
                 (id, library_id, media_type, season_number, primary_id_space, primary_id_value,
                  title, phase, acquisition, presence, release_date_basis, policy_version,
                  next_evaluation_at, created_at, updated_at)
             VALUES (?1, ?2, 'movie', ?3, ?4, ?5, 'Dune: Part Two',
                     'Released', 'Available', 'Real', 'digital', 1, 0, 0, 0)",
            id,
            library_id,
            season_number,
            id_space,
            id_value
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}
