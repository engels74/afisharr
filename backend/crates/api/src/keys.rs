// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Issuing, listing, and revoking API keys.
//!
//! The plaintext is shown once, at creation, and is unrecoverable afterwards.
//! Everything the list surface shows — the name, the prefix, the last-used
//! timestamp — is deliberately not enough to authenticate with.

mod ceiling;
pub(crate) mod routes;

pub use routes::{ApiKeyView, IssuedKey, NewApiKey, create, list, revoke};
