// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Randomness that must be unguessable.
//!
//! Identifiers, session values, API keys, the bootstrap token, and the Argon2id
//! salt all draw from the operating system's CSPRNG, and they draw through one
//! function so there is one place to audit (P7). [`bytes`] panics when the
//! source is unavailable, which is why `secrets` keeps its own fallible draw:
//! there, an absent entropy source is a sealing failure the caller reports, not
//! a reason to stop the process.
//!
//! Seeded, reproducible randomness is a different concern with different rules
//! (PRD §11.4) and does not live here.

mod csprng;

pub use csprng::bytes;
