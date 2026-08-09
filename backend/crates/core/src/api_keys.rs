// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Long-lived credentials for callers that are not a browser.
//!
//! Hashed at rest, shown once at creation, individually revocable, and
//! carrying a last-used timestamp (PRD §19.6). The prefix is stored in the
//! clear so the interface can name a key without holding one.

mod key;
mod store;

pub use key::{ApiKeyRecord, IssuedApiKey, PREFIX_LENGTH};
pub use store::{CreateApiKey, RevokeApiKey, TouchApiKey, find_by_digest, list};
