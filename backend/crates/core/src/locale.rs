// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The interface language, as a value the engine can carry.
//!
//! The formatter registry Phase 3 builds resolves number, date, and list
//! formatting against a locale, so the locale has to be a validated value
//! threaded from settings rather than a free string read at the point of use.
//! Storing it as a newtype now is what makes that a lookup later instead of a
//! migration.

mod tag;

pub use tag::{DEFAULT, LocaleTag, LocaleTagError};
