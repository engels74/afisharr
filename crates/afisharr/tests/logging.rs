// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The application log is a file on disk, rotated, and written from the start.
//!
//! Run against the real binary in a subprocess: logging is initialised by the
//! command-line entry point, so a library call would exercise everything except
//! the wiring under test.

// Integration tests may unwrap: a failed setup step is a failed test, and the
// panic names the line. The rule is about non-test paths (§24.2.3).
#![allow(clippy::unwrap_used)]

use std::process::Command;

use tempfile::TempDir;

#[test]
fn a_start_writes_a_rotated_file_log_under_the_data_directory() {
    let directory = TempDir::new().unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_afisharr"))
        .args(["db", "check"])
        .env("AFISHARR_DATA_DIR", directory.path())
        .output()
        .expect("running the afisharr binary");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let logs: Vec<_> = std::fs::read_dir(directory.path().join("logs"))
        .expect("the log directory must exist after a start")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("afisharr.log")
        })
        .collect();

    assert_eq!(logs.len(), 1, "one rotation period, one file");

    let written = std::fs::read_to_string(logs[0].path()).expect("reading the log");
    assert!(
        written.contains("afisharr started"),
        "the boot line must reach the file: {written}"
    );
}

#[test]
fn the_file_log_is_not_the_run_event_log_the_interface_reads() {
    let directory = TempDir::new().unwrap();

    Command::new(env!("CARGO_BIN_EXE_afisharr"))
        .args(["db", "check"])
        .env("AFISHARR_DATA_DIR", directory.path())
        .output()
        .expect("running the afisharr binary");

    // The text log is for support; the interface reads structured run events
    // from the database (PRD §19.2). Nothing a start writes to the file may
    // have landed in `job_run_events` as a side effect.
    let events = Command::new("sqlite3")
        .arg(directory.path().join("afisharr.db"))
        .arg("SELECT count(*) FROM job_run_events;")
        .output()
        .expect("sqlite3 must be available");

    assert_eq!(String::from_utf8_lossy(&events.stdout).trim(), "0");
}
