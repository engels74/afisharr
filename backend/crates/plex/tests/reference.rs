// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The cross-check: the fake, read by a client that was not written here.
//!
//! Every shape in the adversarial fake is a claim about a server nobody in this
//! repository controls, and the fake and this crate's client were written
//! together, against each other, from one reading of the protocol. They agree
//! by authorship, which is the same failure the fake exists to prevent, one
//! step back.
//!
//! `python-plexapi` is a second reader. It fails on a wrong attribute name
//! whether or not anybody here suspected that name, and it reads XML — which is
//! what a real Plex answers by default and what the fake had never emitted.
//!
//! **This lane needs Python, and the merge lane does not have it.** It runs in
//! the nightly and release lanes. When the reference is not installed this test
//! says so loudly and returns: a cross-check that quietly does not run reads
//! green on the lane that was supposed to catch the drift, which is the same
//! rule the contract test already states for a missing server. The lanes that
//! are supposed to run it check for the interpreter themselves and fail by
//! name when it is absent.

use std::{path::PathBuf, process::Command};

use afisharr_plex::fake::{FakePlex, LibrarySpec, Scenario};

/// Which interpreter to drive the reference with.
const PYTHON: &str = "AFISHARR_PLEX_REFERENCE_PYTHON";

/// The version this repository reads as evidence.
const PINNED: &str = "4.18.2";

/// The script, beside this file.
fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/reference/cross_check.py")
}

/// The interpreter, when one with the reference installed is in reach.
fn interpreter() -> Option<String> {
    let candidate = std::env::var(PYTHON).unwrap_or_else(|_| "python3".to_owned());
    let installed = Command::new(&candidate)
        .args(["-c", "import plexapi; print(plexapi.__version__)"])
        .output()
        .ok()?;
    if !installed.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&installed.stdout).trim().to_owned();
    assert_eq!(
        version, PINNED,
        "the cross-check reads the reference as evidence about a real server, so the \
         version is pinned. Install {PINNED} from tests/reference/requirements.txt, or \
         change the pin deliberately after reading its diff."
    );
    Some(candidate)
}

/// Where the real server is, in the release lane.
const URL: &str = "AFISHARR_PLEX_CONTRACT_URL";

/// The token the release lane supplies for it.
const TOKEN: &str = "AFISHARR_PLEX_CONTRACT_TOKEN";

/// Runs the reference client against one server, and reports what it read.
fn drive(python: &str, base_url: &str, token: &str, read_only: bool) -> std::process::Output {
    let mut command = Command::new(python);
    command
        .arg(script())
        .args(["--base-url", base_url, "--token", token]);
    if read_only {
        command.arg("--read-only");
    }
    command.output().expect("the reference client must run")
}

/// Runs the cross-check against one running fake.
async fn cross_check(scenario: Scenario) {
    let Some(python) = interpreter() else {
        // Not a pass. This line is what a reader of any other lane's log sees
        // instead of a green tick.
        eprintln!(
            "SKIPPED: no reference client. Install plexapi=={PINNED} from \
             backend/crates/plex/tests/reference/requirements.txt, or point {PYTHON} at an \
             interpreter that has it. Without it the fake's shapes are checked only by \
             readers written in this repository (D-036)."
        );
        return;
    };

    let fake = FakePlex::start(scenario).await;
    let base_url = fake.base_url().to_owned();
    // Blocking, and off the runtime's worker threads: the fake is serving this
    // process's own requests from the same runtime, and a blocked worker would
    // deadlock against the server it is waiting on.
    let output =
        tokio::task::spawn_blocking(move || drive(&python, &base_url, "test-plex-token", false))
            .await
            .expect("the reference client task must not panic");

    let report = String::from_utf8_lossy(&output.stdout);
    let complaints = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the reference client could not read the fake.\n{complaints}\n{report}"
    );
    assert!(report.contains(PINNED), "{report}");
    // Printed on the way past, not only on the way down: a lane that says only
    // "2 tests passed" tells whoever reads it nothing about what was read.
    eprintln!("{report}");
}

#[tokio::test]
async fn the_reference_client_reads_every_call_in_the_surface() {
    // `sections()`, `search()`, `collections()`, `collection.items()`,
    // `managedHubs()`, `collection.visibility()`, `listFilters()`,
    // `listFilterChoices()`, `editTags()`, and `uploadPoster()` — each
    // returning a populated object, because a `None` where a value belongs is
    // the failure this lane exists to see.
    cross_check(Scenario::behaving(1)).await;
}

#[test]
fn the_reference_client_reads_the_real_server_too() {
    // A reference client that reads the fake and not the server has proved the
    // fake self-consistent and nothing else. Read-only: this runs against
    // somebody's real Plex, and the writes belong to the contract test, which
    // creates and deletes what it touches (P2).
    let Some(python) = interpreter() else {
        eprintln!("SKIPPED: no reference client for the real-server cross-check.");
        return;
    };
    let (Ok(url), Ok(token)) = (std::env::var(URL), std::env::var(TOKEN)) else {
        eprintln!(
            "SKIPPED: no real Plex server configured. Set {URL} and {TOKEN} to read one \
             with the reference client (D-036)."
        );
        return;
    };
    if url.is_empty() || token.is_empty() {
        eprintln!("SKIPPED: {URL} or {TOKEN} is empty.");
        return;
    }

    let output = drive(&python, &url, &token, true);
    let report = String::from_utf8_lossy(&output.stdout);
    let complaints = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the reference client could not read the real server.\n{complaints}\n{report}"
    );
    eprintln!("{report}");
}

#[tokio::test]
async fn the_reference_client_reads_a_world_it_was_not_built_around() {
    // The same questions of a server with three libraries under keys nothing
    // here hard-codes, so a check that passed by matching the default world's
    // shape fails here.
    cross_check(Scenario::behaving(7).with_libraries([
        LibrarySpec::of("7", "movie", "Films").holding(9),
        LibrarySpec::of("8", "show", "Series").holding(3),
        LibrarySpec::of("9", "artist", "Music").holding(2),
    ]))
    .await;
}
