// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The path boundary, and the one function that decides whether a path is
//! inside it.
//!
//! On an instance that may face the internet (D-029), the browser that walks
//! the asset roots is a path-traversal boundary. Every path is canonicalised
//! and symlink-resolved *before* containment is checked, never before
//! (`I-SEC-3`), and the same function decides placeholder writes against
//! `placeholderRoots` (`I-SEC-4`) — one rule, one implementation (P7).

mod containment;
mod error;
mod listing;
mod roots;

pub use containment::{Contained, Root, contain, contain_new};
pub use error::ContainmentError;
pub use listing::{Entry, EntryKind, list};
pub use roots::enabled as enabled_roots;
