// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `AFISHARR_SECRET_KEY` overrides the key file (D-032).
//!
//! Run against the real binary in a subprocess: `std::env::set_var` is unsafe
//! under edition 2024 and the crate forbids `unsafe`, and a subprocess is the
//! honest shape of the test anyway — this is how an operator mounting the key
//! from a secret manager actually uses it.

// Integration tests may unwrap: a failed setup step is a failed test, and the
// panic names the line. The rule is about non-test paths (§24.2.3).
#![allow(clippy::unwrap_used)]
use std::process::Command;

use afisharr_core::secrets::KEY_ENV_VAR;
use tempfile::TempDir;

#[test]
fn the_environment_override_is_used_instead_of_creating_a_key_file() {
    let directory = TempDir::new().expect("a scratch directory");

    let output = Command::new(env!("CARGO_BIN_EXE_afisharr"))
        .args(["db", "check"])
        .env("AFISHARR_DATA_DIR", directory.path())
        .env(KEY_ENV_VAR, "a".repeat(64))
        .output()
        .expect("running the afisharr binary");

    assert!(
        output.status.success(),
        "the start must succeed with a mounted key: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !directory.path().join("secrets.key").exists(),
        "an override must not also write a key file; two keys is one too many"
    );
}

#[test]
fn a_malformed_override_stops_the_start_rather_than_falling_back_to_a_file() {
    let directory = TempDir::new().expect("a scratch directory");

    let output = Command::new(env!("CARGO_BIN_EXE_afisharr"))
        .args(["db", "check"])
        .env("AFISHARR_DATA_DIR", directory.path())
        .env(KEY_ENV_VAR, "not-a-key")
        .output()
        .expect("running the afisharr binary");

    assert!(
        !output.status.success(),
        "a malformed key must not be silently replaced"
    );
    assert!(
        !directory.path().join("secrets.key").exists(),
        "falling back to a generated key would make every stored credential undecryptable \
         the moment the operator fixed their variable"
    );
}
