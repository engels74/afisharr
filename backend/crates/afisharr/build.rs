// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Makes sure the directory the SPA is embedded from exists.
//!
//! `rust-embed` reads a directory at compile time and fails the build when it
//! is absent. A fresh clone has not run `bun run build` yet, so the directory
//! is created empty here and the binary reports "this build carries no
//! interface" at runtime — which is a true statement an operator can act on,
//! rather than a compile error in a Rust crate about a missing frontend.

use std::path::PathBuf;

fn main() {
    let build = spa_directory();
    if let Err(error) = std::fs::create_dir_all(&build) {
        // Not a hard failure: a read-only checkout can still build the backend,
        // and rust-embed will say so itself if the directory really is missing.
        println!(
            "cargo:warning=could not create {}: {error}",
            build.display()
        );
    }
    // Rebuild when the SPA changes, so `cargo build` after `bun run build`
    // embeds the new bundle instead of the one from the last compile.
    println!("cargo:rerun-if-changed={}", build.display());
    println!("cargo:rerun-if-changed=build.rs");
}

/// `frontend/build`, resolved from this crate's manifest directory.
fn spa_directory() -> PathBuf {
    let manifest = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("cargo always sets CARGO_MANIFEST_DIR"),
    );
    manifest
        .join("..")
        .join("..")
        .join("..")
        .join("frontend")
        .join("build")
}
