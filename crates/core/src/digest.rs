// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Turning a value into a stable digest.
//!
//! Every `_hash` and `_sha256` column in the schema is lowercase hex over a
//! canonical form (PRD §19.1). Canonical means the digest depends on what the
//! document says and not on how it was written — otherwise re-saving an
//! unchanged definition produces a new hash, which makes the concurrency token
//! useless and the render cache miss for no reason.

mod canonical_json;
mod sha256;

pub use canonical_json::canonicalize;
pub use sha256::hex;
