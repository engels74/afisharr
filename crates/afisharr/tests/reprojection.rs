// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Derived columns are recomputable, and reprojection is a no-op when they are.

// Integration tests may unwrap: a failed setup step is a failed test, and the
// panic names the line. The rule is about non-test paths (§24.2.3).
#![allow(clippy::unwrap_used)]
mod harness;

use afisharr_core::{
    projection::{self, project_definition},
    storage::{WriteHandle, WriteOperation},
};
use harness::TempInstance;
use sqlx::SqliteConnection;

const BODY: &str = r#"{
    "kind": "Collection",
    "schemaVersion": 1,
    "registryVersion": 3,
    "id": "01J9Z7Q0K8Y3X2W1V0U9T8S7R6",
    "handle": "user/trending-now",
    "name": "Trending Now",
    "meta": { "origin": { "type": "user" }, "tags": [] },
    "spec": {}
}"#;

#[tokio::test]
async fn reprojection_is_a_no_op_against_an_empty_database() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;

    let report = projection::reproject(booted.database.readers(), booted.database.writer())
        .await
        .expect("reprojecting");

    assert!(report.is_noop());
    assert_eq!(report.definitions_checked, 0);
    booted.database.close().await;
}

#[tokio::test]
async fn reprojection_is_a_no_op_against_a_populated_database() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;
    seed_definitions(booted.database.writer(), 3).await;

    let first = projection::reproject(booted.database.readers(), booted.database.writer())
        .await
        .expect("reprojecting");

    assert_eq!(first.definitions_checked, 3);
    assert!(
        first.is_noop(),
        "rows written through the projection must not need correcting"
    );

    // Running it twice is the honest form of the check: a sweep that corrects
    // on the first pass and not the second is a sweep that was writing, not a
    // projection that agreed.
    let second = projection::reproject(booted.database.readers(), booted.database.writer())
        .await
        .expect("reprojecting again");
    assert!(second.is_noop());

    booted.database.close().await;
}

#[tokio::test]
async fn a_derived_column_written_by_anything_else_is_corrected_back() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;
    seed_definitions(booted.database.writer(), 1).await;

    booted
        .database
        .writer()
        .submit(CorruptDerivedColumns)
        .await
        .expect("staging the drift");

    let report = projection::reproject(booted.database.readers(), booted.database.writer())
        .await
        .expect("reprojecting");
    assert_eq!(report.definitions_corrected, 1);

    let name: String = sqlx::query_scalar("SELECT name FROM definitions LIMIT 1")
        .fetch_one(booted.database.readers())
        .await
        .expect("reading the corrected row");
    assert_eq!(
        name, "Trending Now",
        "the body is the source of truth, not the column"
    );

    let after = projection::reproject(booted.database.readers(), booted.database.writer())
        .await
        .expect("reprojecting again");
    assert!(after.is_noop(), "one sweep reaches a fixed point");

    booted.database.close().await;
}

#[tokio::test]
async fn the_reproject_command_reports_a_no_op_and_exits_zero() {
    let directory = tempfile::TempDir::new().expect("a scratch directory");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_afisharr"))
        .args(["db", "reproject"])
        .env("AFISHARR_DATA_DIR", directory.path())
        .output()
        .expect("running the afisharr binary");

    assert!(
        output.status.success(),
        "db reproject must exit zero: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("no drift found"), "{stdout}");
}

async fn seed_definitions(writer: &WriteHandle, count: usize) {
    for index in 0..count {
        let id = format!("DEF{index:023}");
        let body = BODY.replace("user/trending-now", &format!("user/trending-now-{index}"));
        writer
            .submit(InsertDefinition { id, body })
            .await
            .expect("seeding a definition");
    }
}

/// Inserts a definition with its derived columns written by the projection.
struct InsertDefinition {
    id: String,
    body: String,
}

impl WriteOperation for InsertDefinition {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let columns = project_definition(&self.id, &self.body)
            .map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
        let Self { id, body } = self;

        sqlx::query!(
            "INSERT INTO definitions
                 (id, kind, handle, name, schema_version, registry_version, body_json, body_hash,
                  origin_kind, origin_pack, origin_pack_version, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, 0)",
            id,
            columns.kind,
            columns.handle,
            columns.name,
            columns.schema_version,
            columns.registry_version,
            body,
            columns.body_hash,
            columns.origin_kind,
            columns.origin_pack,
            columns.origin_pack_version
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

/// Writes a derived column directly — the thing the rule forbids — so the sweep
/// has something to find.
struct CorruptDerivedColumns;

impl WriteOperation for CorruptDerivedColumns {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        sqlx::query!("UPDATE definitions SET name = 'Edited behind the projection'")
            .execute(&mut *conn)
            .await?;
        Ok(())
    }
}
