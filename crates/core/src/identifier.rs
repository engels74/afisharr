// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The identifiers Afisharr assigns to itself.
//!
//! Every entity Afisharr creates carries a ULID primary key (PRD §19.1).
//! Identifiers Plex or a provider assigns are bindings on the row, never keys —
//! they change, and reconciliation rebinds them (P4).

mod id;
mod principals;

pub use id::{Id, IdError};
pub use principals::{EVERYONE, OWNER, SHARED_ALL};
