// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `afisharr openapi`.

use anyhow::{Context, Result};

/// Writes the `OpenAPI` document to stdout.
///
/// The client generator reads this. It is a subcommand rather than a build
/// script because the document is a function of the compiled binary's
/// annotations: generating it any other way would be a second description of
/// the surface, and the one that drifts is the one nobody runs (§24.5).
///
/// Deliberately available before setup and without a credential: this is the
/// contract, not the operator's data, and the `contract-check` lane runs it in
/// CI against a database that does not exist.
///
/// # Errors
/// Returns an error when the document cannot be serialised, which can only
/// mean a malformed annotation on a handler.
pub fn run() -> Result<()> {
    let document = afisharr_api::openapi::document().context("serialising the OpenAPI document")?;
    println!("{document}");
    Ok(())
}
