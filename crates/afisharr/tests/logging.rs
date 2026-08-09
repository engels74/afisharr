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

use afisharr_core::settings::LoggingSettings;
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

#[test]
fn an_empty_log_level_override_stops_the_start_rather_than_silencing_the_log() {
    let directory = TempDir::new().unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_afisharr"))
        .args(["db", "check"])
        .env("AFISHARR_DATA_DIR", directory.path())
        .env("AFISHARR_LOG_LEVEL", "")
        .output()
        .expect("running the afisharr binary");

    // `EnvFilter::try_new("")` parses and yields `LevelFilter::OFF`, so an empty
    // variable is the one value that turns the support log off without saying
    // anything. A compose file writes it whenever the value it meant to
    // interpolate was missing.
    assert!(
        !output.status.success(),
        "an empty override must not be taken as a filter"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("AFISHARR_LOG_LEVEL"), "{stderr}");
}

#[test]
fn the_rotated_log_keeps_only_the_configured_number_of_files() {
    let directory = TempDir::new().unwrap();
    let logs = directory.path().join("logs");
    std::fs::create_dir_all(&logs).unwrap();

    // Rotations this process did not write, standing in for previous days. A
    // year long past, so the clock the test runs under can never be one of
    // them. `logging.retainedFiles` defaults to seven, so twelve is too many.
    for day in 1..=12 {
        std::fs::write(
            logs.join(format!("afisharr.log.2020-01-{day:02}")),
            b"old\n",
        )
        .unwrap();
    }

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

    let mut kept: Vec<String> = std::fs::read_dir(&logs)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("afisharr.log"))
        .collect();
    kept.sort();

    let retained = usize::from(LoggingSettings::default().retained_files);
    assert_eq!(
        kept.len(),
        retained,
        "`logging.retainedFiles` is a promise about the directory, not a field nobody \
         reads: {kept:?}"
    );
    assert_eq!(
        kept.iter().filter(|name| name.contains("2020-01-")).count(),
        retained - 1,
        "one of the kept files is the one being written: {kept:?}"
    );
}

#[test]
fn an_empty_override_of_any_field_stops_the_start() {
    // One variable per field is a hand-written list, so the refusal is asserted
    // somewhere other than on the field that motivated it.
    let directory = TempDir::new().unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_afisharr"))
        .args(["db", "check"])
        .env("AFISHARR_DATA_DIR", directory.path())
        .env("AFISHARR_TIMEZONE", "")
        .output()
        .expect("running the afisharr binary");

    assert!(
        !output.status.success(),
        "an empty timezone is not a timezone"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("AFISHARR_TIMEZONE"), "{stderr}");
}

#[test]
fn an_empty_data_directory_override_stops_the_start() {
    // Run somewhere disposable: the value under test is the one that resolves
    // against the working directory, and a regression would otherwise write a
    // database and an instance key into the package being tested.
    let elsewhere = TempDir::new().unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_afisharr"))
        .args(["db", "check"])
        .current_dir(elsewhere.path())
        .env("AFISHARR_DATA_DIR", "")
        .output()
        .expect("running the afisharr binary");

    // Taking it would put the database, the instance key, and the backups in
    // the working directory — outside the mount, and outside what an operator
    // backs up.
    assert!(
        !output.status.success(),
        "an empty data directory must not resolve to the working directory"
    );
    assert!(
        !elsewhere.path().join("data").exists(),
        "nothing may be written against the working directory"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("AFISHARR_DATA_DIR"), "{stderr}");
}
