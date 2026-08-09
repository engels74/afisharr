// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `I-DATA-2` — no transaction spans external I/O.
//!
//! The architectural half of the invariant, checked over the source tree rather
//! than at runtime. The structural argument is that a mutation is a
//! [`WriteOperation`](afisharr_core::storage::WriteOperation) which receives a
//! connection and nothing else, so it has nothing to make an external call
//! with. This test is what stops that argument quietly becoming untrue: a file
//! that opens a transaction, or implements a write operation, may not also
//! reach the network or the filesystem.
//!
//! A hung socket inside an open write transaction blocks every other writer for
//! the length of a timeout, which presents as the whole application freezing.

use std::path::{Path, PathBuf};

/// Calls that leave the process. A file that opens a transaction may not use one.
const EXTERNAL_IO: [&str; 6] = [
    "reqwest",
    "tokio::fs",
    "tokio::net",
    "std::fs::",
    "std::net::",
    "spawn_blocking",
];

/// Markers that mean "this file holds a transaction open, or could".
const TRANSACTIONAL: [&str; 3] = ["impl WriteOperation for", ".begin(", "Connection::begin("];

#[test]
fn no_file_holds_a_transaction_across_external_io() {
    let mut offences = Vec::new();

    for file in rust_sources(&workspace_root()) {
        let source = std::fs::read_to_string(&file).expect("reading a source file");
        let source = without_test_module(&source);

        if !TRANSACTIONAL.iter().any(|marker| source.contains(marker)) {
            continue;
        }

        for call in EXTERNAL_IO {
            if source.contains(call) {
                offences.push(format!(
                    "{}: transactional code calls {call}",
                    file.display()
                ));
            }
        }
    }

    assert!(
        offences.is_empty(),
        "a transaction may not span external I/O (PRD §19.4):\n{}",
        offences.join("\n")
    );
}

#[test]
fn the_scan_actually_reaches_the_files_it_claims_to_check() {
    // A scanner that silently matches nothing passes forever. This asserts the
    // walk finds the module that does hold transactions open.
    let found: Vec<PathBuf> = rust_sources(&workspace_root())
        .into_iter()
        .filter(|path| path.ends_with("settings/store.rs"))
        .collect();
    assert_eq!(
        found.len(),
        1,
        "expected to scan crates/core/src/settings/store.rs"
    );
}

/// The workspace root, two levels above this crate's manifest.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate manifest sits two levels below the workspace root")
        .to_path_buf()
}

/// Every non-test Rust source under `crates/*/src`.
fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.join("crates")];

    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "target") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs")
                && path
                    .components()
                    .any(|component| component.as_os_str() == "src")
            {
                found.push(path);
            }
        }
    }
    found
}

/// Everything before the in-module test block.
///
/// Test code legitimately reads the filesystem to build fixtures; the rule is
/// about the paths that run in production.
fn without_test_module(source: &str) -> &str {
    source.split("#[cfg(test)]").next().unwrap_or(source)
}
