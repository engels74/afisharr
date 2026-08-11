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
    let (instance_id, client_identifier, first_started_at) = (
        first.instance.instance_id.clone(),
        first.instance.client_identifier.clone(),
        first.instance.first_started_at,
    );
    first.database.close().await;

    let second = instance.boot().await;
    assert_eq!(second.instance.instance_id, instance_id);
    assert_eq!(
        second.instance.client_identifier, client_identifier,
        "plex.tv binds tokens to this value; regenerating it orphans every one of them"
    );
    assert_eq!(
        second.instance.first_started_at, first_started_at,
        "the installation's first start is recorded once, not reset by every restart"
    );
    second.database.close().await;
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

/// Retention says how much history to keep, never whether *this* migration is
/// protected: the copy taken moments earlier is not a prune candidate.
#[tokio::test]
async fn a_retention_of_zero_still_leaves_this_migration_its_backup() {
    let instance = TempInstance::new();
    let mut configured = SettingsBody::default();
    configured.backup.retained_pre_migration = 0;

    let booted = instance.boot_with(configured).await;
    booted.database.close().await;

    let copies = std::fs::read_dir(instance.paths().backups())
        .expect("the backup directory must exist after a migration")
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
        "a forward-only migration may never run with nothing behind it (I-DATA-8)"
    );
}

/// A deployment variable has to survive the first boot, or it does nothing.
///
/// The failure this pins: `ensure_settings` returned the stored `settings` row
/// verbatim, so every `AFISHARR_*` deployment variable was read exactly once —
/// on the very first start a container ever made — and ignored on every start
/// after it. An operator upgrading an existing instance, adding
/// `AFISHARR_PUBLIC_ORIGIN` to their compose file and restarting, was told by
/// the hosted Plex sign-in to set the setting they had just set;
/// `AFISHARR_TRUST_PROXY` behind a reverse proxy was ignored the same way, so
/// every request kept being attributed to the proxy's own address and twenty
/// failed sign-ins from any one visitor rate-limited the whole instance.
///
/// Driven through the real binary and a real socket, because the claim is about
/// what a container operator gets: the port is the observable that no library
/// call can fake, and it answers only if the environment beat the stored row.
#[tokio::test]
async fn an_environment_override_is_honoured_on_every_start_and_not_only_the_first() {
    let instance = TempInstance::new();

    // A first start, which is what writes the `settings` row. It stores the
    // default port, and from here on that row is the one being overridden.
    instance.boot().await.database.close().await;

    let port = free_port();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_afisharr"))
        .arg("start")
        .env("AFISHARR_DATA_DIR", instance.paths().root())
        .env("AFISHARR_BIND_ADDRESS", "127.0.0.1")
        .env("AFISHARR_PORT", port.to_string())
        .spawn()
        .expect("the afisharr binary must start");

    let answered = wait_for_health(port).await;

    let _ = child.kill();
    let _ = child.wait();

    assert!(
        answered,
        "nothing answered on port {port}: the stored settings row won over \
         AFISHARR_PORT, so every deployment variable is dead after the first boot"
    );
}

/// A deployment variable may not edit what the operator saved.
///
/// The other half of the rule above, and the half that made the promise false.
/// `ensure_settings` laid the *whole* override list back over the stored row,
/// and three of those overrides are not deployment shape at all: `timezone`,
/// `locale` and `device_name` are written into the persisted `instance` row by
/// `ensure_instance` on every start. So an `AFISHARR_TIMEZONE=UTC` left in a
/// compose template — a common default — reverted the operator's saved zone at
/// every restart, the engine's day-aligned date operators then ran in it, and
/// the settings page went on showing the value they had chosen. The same path
/// renamed the instance in their plex.tv device list.
///
/// Driven through the real binary, because the claim is about what a restart
/// with that variable still set actually leaves behind in the database.
#[tokio::test]
async fn an_environment_seed_does_not_revert_an_instance_field_the_operator_saved() {
    let instance = TempInstance::new();

    // The saved state: a first start seeds `settings` and `instance` with a
    // zone that is not the default, standing in for the operator choosing one.
    let mut saved = SettingsBody::default();
    saved.instance.timezone = "Europe/Copenhagen".to_owned();
    let booted = instance.boot_with(saved).await;
    assert_eq!(booted.instance.timezone, "Europe/Copenhagen");
    booted.database.close().await;

    let port = free_port();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_afisharr"))
        .arg("start")
        .env("AFISHARR_DATA_DIR", instance.paths().root())
        .env("AFISHARR_BIND_ADDRESS", "127.0.0.1")
        .env("AFISHARR_PORT", port.to_string())
        .env("AFISHARR_TIMEZONE", "UTC")
        .spawn()
        .expect("the afisharr binary must start");

    let answered = wait_for_health(port).await;

    let _ = child.kill();
    let _ = child.wait();
    assert!(answered, "nothing answered on port {port}");

    // Read out of the file rather than through another boot. A boot rewrites
    // the `instance` row from the stored settings, so it would repair exactly
    // the damage this is looking for and the assertion could never fail.
    let opened = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}?mode=ro",
        instance.paths().database().display()
    ))
    .await
    .expect("the database must open");
    let timezone: String = sqlx::query_scalar("SELECT timezone FROM instance WHERE id = 1")
        .fetch_one(&opened)
        .await
        .expect("the instance row must exist");
    opened.close().await;

    assert_eq!(
        timezone, "Europe/Copenhagen",
        "a compose variable overwrote a persisted field the operator saved"
    );
}

/// A port nothing is listening on, as far as the operating system knows.
fn free_port() -> u16 {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("binding an ephemeral port must succeed");
    let port = listener
        .local_addr()
        .expect("a bound listener has an address")
        .port();
    drop(listener);
    port
}

/// Whether the health route answers on `port` within a bounded wait.
///
/// Polled rather than slept on: a fixed sleep is either longer than the test
/// needs on every run or shorter than it needs on a loaded machine, and the
/// second of those is a test that fails for a reason nobody can reproduce.
async fn wait_for_health(port: u16) -> bool {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{port}/api/health");
    for _ in 0..100 {
        if let Ok(response) = client.get(&url).send().await
            && response.status().is_success()
        {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    false
}
