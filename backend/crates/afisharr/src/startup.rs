// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What happens on every boot, in the order PRD §19.3 fixes.
//!
//! Open, refuse a schema this binary does not know, back up before migrating,
//! migrate, verify, then reconcile what a previous run left behind.

mod migrations;
mod reconcile;
mod sequence;

pub use migrations::{MIGRATOR, MigrationState, inspect};
pub use sequence::{Booted, boot};
