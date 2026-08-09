// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What migration `0001` produces, and the two structural promises it makes.

// Integration tests may unwrap: a failed setup step is a failed test, and the
// panic names the line. The rule is about non-test paths (§24.2.3).
#![allow(clippy::unwrap_used)]
mod harness;

use afisharr_core::identifier::{EVERYONE, OWNER, SHARED_ALL};
use harness::{InsertLifecycleSubject, InsertPlexPrincipal, InsertVisibility, TempInstance};

/// PRD §19 defines 68 tables; `_sqlx_migrations` is sqlx's bookkeeping, not ours.
const DEFINED_TABLES: i64 = 68;

#[tokio::test]
async fn a_fresh_database_carries_every_defined_table() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;

    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> '_sqlx_migrations'",
    )
    .fetch_one(booted.database.readers())
    .await
    .expect("counting tables");

    assert_eq!(tables, DEFINED_TABLES);
    booted.database.close().await;
}

#[tokio::test]
async fn the_one_way_door_pragmas_are_in_force_on_the_created_file() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;
    let readers = booted.database.readers();

    let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
        .fetch_one(readers)
        .await
        .unwrap();
    let auto_vacuum: i64 = sqlx::query_scalar("PRAGMA auto_vacuum")
        .fetch_one(readers)
        .await
        .unwrap();
    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(readers)
        .await
        .unwrap();

    assert_eq!(
        page_size, 8192,
        "page_size can only be chosen before the first write"
    );
    assert_eq!(auto_vacuum, 2, "2 is INCREMENTAL");
    assert_eq!(journal_mode, "wal");
    booted.database.close().await;
}

#[tokio::test]
async fn the_three_whole_audience_principals_are_seeded_with_their_fixed_identifiers() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;

    let seeded: Vec<(String, String)> =
        sqlx::query_as("SELECT id, kind FROM principals ORDER BY id")
            .fetch_all(booted.database.readers())
            .await
            .expect("reading the seeded principals");

    assert_eq!(
        seeded,
        vec![
            (EVERYONE.to_owned(), "Everyone".to_owned()),
            (OWNER.to_owned(), "Owner".to_owned()),
            (SHARED_ALL.to_owned(), "SharedAll".to_owned()),
        ]
    );
    booted.database.close().await;
}

/// `I-DATA-5` — per-user targeting requires no schema migration.
///
/// Nothing in Tier 0 writes a `PlexUser` principal; the point of the test is
/// that the launch schema already accepts one, so Tier 1 is a widening rather
/// than a migration on live data.
#[tokio::test]
async fn a_per_user_principal_and_a_visibility_row_insert_against_the_launch_schema() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;
    let writer = booted.database.writer();

    let library = harness::seed_library(writer, "LIB0000000000000000000001").await;
    let principal = "PRIN000000000000000000001".to_owned() + "X";

    writer
        .submit(InsertPlexPrincipal {
            id: principal.clone(),
            plex_account_id: 4_242,
        })
        .await
        .expect("a PlexUser principal must insert with no ALTER TABLE");

    writer
        .submit(InsertVisibility {
            participant_id: "PART0000000000000000000001".to_owned(),
            library_id: library,
            principal_id: principal.clone(),
        })
        .await
        .expect("a visibility row referencing it must insert with no ALTER TABLE");

    let targeted: i64 =
        sqlx::query_scalar("SELECT count(*) FROM placement_visibility WHERE principal_id = ?1")
            .bind(&principal)
            .fetch_one(booted.database.readers())
            .await
            .expect("counting visibility rows");

    assert_eq!(targeted, 1);
    booted.database.close().await;
}

/// `I-DATA-10` — one lifecycle subject per identity, enforced by the database.
#[tokio::test]
async fn a_second_whole_title_subject_for_one_identity_is_rejected_by_the_index() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;
    let writer = booted.database.writer();

    let library = harness::seed_library(writer, "LIB0000000000000000000001").await;
    let subject = |id: &str| InsertLifecycleSubject {
        id: id.to_owned(),
        library_id: library.clone(),
        id_space: "tmdb".to_owned(),
        id_value: "693134".to_owned(),
        season_number: None,
    };

    writer
        .submit(subject("SUBJ000000000000000000001"))
        .await
        .expect("the first subject inserts");

    let refusal = writer
        .submit(subject("SUBJ000000000000000000002"))
        .await
        .expect_err("a second subject for the same identity must be refused");

    let database_error = match &refusal {
        afisharr_core::storage::StorageError::Statement(error) => error
            .as_database_error()
            .expect("the refusal must carry SQLite's own error, not a wrapper"),
        other => panic!("the refusal must come from the statement: {other:?}"),
    };
    assert!(
        database_error.is_unique_violation(),
        "the refusal must come from the index, not from application code: {database_error:?}"
    );
    assert!(
        database_error
            .message()
            .contains("ux_lifecycle_subjects__identity"),
        "the refusal must name the identity index rather than any other unique constraint: \
         {database_error:?}"
    );

    let subjects: i64 = sqlx::query_scalar("SELECT count(*) FROM lifecycle_subjects")
        .fetch_one(booted.database.readers())
        .await
        .expect("counting subjects");
    assert_eq!(subjects, 1, "exactly one row survives the race");
    booted.database.close().await;
}

/// The same identity in a different season is a different subject (D-025).
#[tokio::test]
async fn a_season_subject_coexists_with_the_whole_title_subject() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;
    let writer = booted.database.writer();

    let library = harness::seed_library(writer, "LIB0000000000000000000001").await;
    for (id, season) in [
        ("SUBJ000000000000000000001", None),
        ("SUBJ000000000000000000002", Some(2)),
    ] {
        writer
            .submit(InsertLifecycleSubject {
                id: id.to_owned(),
                library_id: library.clone(),
                id_space: "tmdb".to_owned(),
                id_value: "693134".to_owned(),
                season_number: season,
            })
            .await
            .expect("a whole title and one of its seasons are different subjects");
    }

    let subjects: i64 = sqlx::query_scalar("SELECT count(*) FROM lifecycle_subjects")
        .fetch_one(booted.database.readers())
        .await
        .expect("counting subjects");
    assert_eq!(subjects, 2);
    booted.database.close().await;
}
