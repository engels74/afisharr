// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The boot sequence: refuse, back up, migrate, verify, reconcile.

// Integration tests may unwrap: a failed setup step is a failed test, and the
// panic names the line. The rule is about non-test paths (§24.2.3).
#![allow(clippy::unwrap_used)]
mod harness;

use afisharr_core::{backup::PRE_MIGRATION_PREFIX, integrity, settings::SettingsBody};
use harness::TempInstance;

#[tokio::test]
async fn a_first_start_migrates_and_passes_both_integrity_checks() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;

    let report = integrity::verify(booted.database.readers())
        .await
        .expect("running the integrity checks");

    assert!(
        report.is_clean(),
        "foreign_key_check and integrity_check must be clean on first start: {report:?}"
    );
    booted.database.close().await;
}

#[tokio::test]
async fn a_pending_migration_writes_its_backup_before_it_runs() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;
    booted.database.close().await;

    let copies: Vec<_> = std::fs::read_dir(instance.paths().backups())
        .expect("the backup directory must exist after a migration")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(PRE_MIGRATION_PREFIX)
        })
        .collect();

    assert_eq!(
        copies.len(),
        1,
        "one pending migration, one pre-migration copy"
    );

    // Taken through the online backup API, so the copy is a database rather
    // than a byte-for-byte snapshot of a file that was being written.
    let copy = copies[0].path();
    let opened = sqlx::SqlitePool::connect(&format!("sqlite://{}?mode=ro", copy.display()))
        .await
        .expect("the copy must open as a database");
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'principals'",
    )
    .fetch_one(&opened)
    .await
    .expect("reading the copy");
    opened.close().await;

    // The copy is taken *before* the migration, so on a first start it holds
    // the empty database sqlx had just created its bookkeeping table in.
    assert_eq!(tables, 0, "the copy predates the migration it protects");
}

#[tokio::test]
async fn a_second_start_finds_nothing_pending_and_takes_no_further_backup() {
    let instance = TempInstance::new();
    instance.boot().await.database.close().await;
    instance.boot().await.database.close().await;

    let copies = std::fs::read_dir(instance.paths().backups())
        .expect("the backup directory")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(PRE_MIGRATION_PREFIX)
        })
        .count();

    assert_eq!(
        copies, 1,
        "a start with no pending migration backs nothing up"
    );
}

#[tokio::test]
async fn the_client_identifier_survives_a_restart_unchanged() {
    let instance = TempInstance::new();

    let first = instance.boot().await;
    let (instance_id, client_identifier) = (
        first.instance.instance_id.clone(),
        first.instance.client_identifier.clone(),
    );
    first.database.close().await;

    let second = instance.boot().await;
    assert_eq!(second.instance.instance_id, instance_id);
    assert_eq!(
        second.instance.client_identifier, client_identifier,
        "plex.tv binds tokens to this value; regenerating it orphans every one of them"
    );
    assert_eq!(second.instance.first_started_at, first_started_at(&second));
    second.database.close().await;
}

fn first_started_at(booted: &afisharr::startup::Booted) -> afisharr_core::time::Timestamp {
    booted.instance.first_started_at
}

#[tokio::test]
async fn settings_are_seeded_once_and_the_stored_row_wins_afterwards() {
    let instance = TempInstance::new();

    let first = instance.boot().await;
    assert_eq!(first.settings.version, 1);
    assert_eq!(first.settings.body, SettingsBody::default());
    first.database.close().await;

    // A second start does not re-seed: the operator edits settings through the
    // interface, and a config file that silently overwrote those edits on the
    // next restart would make the settings page lie about what is running.
    let second = instance.boot().await;
    assert_eq!(second.settings.version, 1);

    let versions: i64 = sqlx::query_scalar("SELECT count(*) FROM settings_history")
        .fetch_one(second.database.readers())
        .await
        .expect("counting settings history");
    assert_eq!(
        versions, 1,
        "every settings write lands as one whole versioned body"
    );
    second.database.close().await;
}

#[tokio::test]
async fn the_secret_key_file_is_created_once_and_reused() {
    let instance = TempInstance::new();

    let first = instance.boot().await;
    let key_path = instance.paths().secret_key();
    assert!(
        key_path.exists(),
        "a first start creates the key beside the database"
    );
    let key_bytes = std::fs::read(&key_path).expect("reading the key");
    assert_eq!(key_bytes.len(), 32);
    first.database.close().await;

    let second = instance.boot().await;
    assert_eq!(
        std::fs::read(&key_path).expect("reading the key"),
        key_bytes
    );
    second.database.close().await;
}

#[cfg(unix)]
#[tokio::test]
async fn the_secret_key_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let instance = TempInstance::new();
    let booted = instance.boot().await;

    let mode = std::fs::metadata(instance.paths().secret_key())
        .expect("the key file")
        .permissions()
        .mode()
        & 0o777;

    assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
    booted.database.close().await;
}

#[tokio::test]
async fn a_secret_round_trips_and_is_unreadable_without_the_key() {
    use afisharr_core::{
        secrets::{self, PutSecret, SecretKey},
        time::Timestamp,
    };

    let instance = TempInstance::new();
    let booted = instance.boot().await;

    let sealed = booted
        .secret_key
        .seal(b"plex-token-value")
        .expect("sealing");
    booted
        .database
        .writer()
        .submit(PutSecret {
            name: "plex.token".to_owned(),
            sealed,
            at: Timestamp::from_millis(1),
        })
        .await
        .expect("storing the secret");

    let recovered = secrets::get(booted.database.readers(), &booted.secret_key, "plex.token")
        .await
        .expect("reading the secret");
    assert_eq!(recovered.as_deref(), Some(b"plex-token-value".as_slice()));

    // A database copied without `secrets.key` decrypts nothing: the stored
    // value is unobservable, which is reported as such rather than as absent.
    let other_key = SecretKey::generate().expect("a key");
    let refusal = secrets::get(booted.database.readers(), &other_key, "plex.token")
        .await
        .expect_err("a foreign key must not decrypt the row");
    assert!(format!("{refusal}").contains("plex.token"));

    booted.database.close().await;
}

/// A database copied without `secrets.key` decrypts nothing.
///
/// The copy starts cleanly — it mints its own key, because a start that refused
/// would make a restore unrecoverable — and every stored secret is then
/// *unobservable* rather than absent. Nothing may be deleted on the strength of
/// a secret that will not decrypt (P1, PRD §21.6.3).
#[tokio::test]
async fn a_database_copied_without_the_key_cannot_decrypt_any_secret() {
    use afisharr_core::{secrets::PutSecret, time::Timestamp};

    let original = TempInstance::new();
    let booted = original.boot().await;
    let sealed = booted
        .secret_key
        .seal(b"plex-token-value")
        .expect("sealing");
    booted
        .database
        .writer()
        .submit(PutSecret {
            name: "plex.token".to_owned(),
            sealed,
            at: Timestamp::from_millis(1),
        })
        .await
        .expect("storing the secret");
    booted.database.close().await;

    // Copy the database and leave the key behind, which is what the default
    // backup does (PRD §21.6.1).
    let elsewhere = TempInstance::new();
    std::fs::copy(original.paths().database(), elsewhere.paths().database())
        .expect("copying the database");
    assert!(!elsewhere.paths().secret_key().exists());

    let restored = elsewhere.boot().await;
    assert!(
        elsewhere.paths().secret_key().exists(),
        "a fresh key is minted rather than the start failing; a restore that refuses to \
         boot is a restore nobody can finish"
    );

    let refusal = afisharr_core::secrets::get(
        restored.database.readers(),
        &restored.secret_key,
        "plex.token",
    )
    .await
    .expect_err("the copied row must not decrypt under a different key");
    assert!(format!("{refusal}").contains("plex.token"));

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM secrets")
        .fetch_one(restored.database.readers())
        .await
        .expect("counting secrets");
    assert_eq!(rows, 1, "an unreadable secret is not a deleted one");

    restored.database.close().await;
}

#[tokio::test]
async fn no_secret_value_reaches_settings_or_its_history() {
    use afisharr_core::{secrets::PutSecret, time::Timestamp};

    let instance = TempInstance::new();
    let booted = instance.boot().await;

    let sealed = booted
        .secret_key
        .seal(b"super-secret-token")
        .expect("sealing");
    booted
        .database
        .writer()
        .submit(PutSecret {
            name: "plex.token".to_owned(),
            sealed,
            at: Timestamp::from_millis(1),
        })
        .await
        .expect("storing the secret");

    let bodies: Vec<String> = sqlx::query_scalar(
        "SELECT body_json FROM settings
         UNION ALL SELECT body_json FROM settings_history
         UNION ALL SELECT IFNULL(diff_json, '') FROM settings_history",
    )
    .fetch_all(booted.database.readers())
    .await
    .expect("reading every settings body");

    for body in bodies {
        assert!(
            !body.contains("super-secret-token"),
            "a secret reached the settings body: {body}"
        );
        assert!(
            !body.contains("plex.token"),
            "a secret name reached the settings body: {body}"
        );
    }
    booted.database.close().await;
}
