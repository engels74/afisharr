// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `I-DATA-7` — a newer schema is never opened by an older binary.
//!
//! Run against the real binary in a subprocess, because the property under test
//! is that the *process* refuses to start, and a library call that returns an
//! error proves only half of that.

// Integration tests may unwrap: a failed setup step is a failed test, and the
// panic names the line. The rule is about non-test paths (§24.2.3).
#![allow(clippy::unwrap_used)]
use std::process::Command;

use tempfile::TempDir;

#[test]
fn a_database_at_an_unknown_migration_version_refuses_to_start() {
    let directory = TempDir::new().expect("a scratch directory");

    // A clean start first, so the database exists and is fully migrated.
    let first = run_check(directory.path());
    assert!(
        first.status.success(),
        "the first start must succeed: {}",
        first.stderr
    );

    // Then record a migration from a future release. Nothing else changes: the
    // tables this binary knows are all still there, which is exactly the case
    // that makes a silent downgrade dangerous.
    let database = directory.path().join("afisharr.db");
    let recorded = Command::new("sqlite3")
        .arg(&database)
        .arg(
            "INSERT INTO _sqlx_migrations
                 (version, description, success, checksum, execution_time)
             VALUES (99, 'from a future release', 1, X'00', 0);",
        )
        .status()
        .expect("sqlite3 must be available to stage the future migration");
    assert!(recorded.success());

    let second = run_check(directory.path());

    assert!(
        !second.status.success(),
        "an older binary must refuse a newer schema"
    );
    assert!(
        second.stderr.contains("99"),
        "the message must name the version found: {}",
        second.stderr
    );
    assert!(
        second.stderr.contains("does not know")
            && second.stderr.contains("newest migration it carries is"),
        "the message must name the newest version this binary carries: {}",
        second.stderr
    );
}

struct Run {
    status: std::process::ExitStatus,
    stderr: String,
}

fn run_check(data_dir: &std::path::Path) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_afisharr"))
        .args(["db", "check"])
        .env("AFISHARR_DATA_DIR", data_dir)
        .output()
        .expect("running the afisharr binary");

    Run {
        status: output.status,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}
