// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The two checks that run on the first start after a migration.
//!
//! PRD §19.3 makes both mandatory: `foreign_key_check` catches a reference a
//! table rebuild broke while foreign keys were off, and `integrity_check`
//! catches structural damage. Neither is expensive enough to be worth skipping
//! at the one moment the schema has just changed.

mod checks;

pub use checks::{IntegrityReport, verify};
