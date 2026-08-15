// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Filter-metadata discovery: the vocabulary the server itself declares.
//!
//! PRD §13.2.4 makes the field registry two layers, and this is the one that is
//! fetched rather than compiled: per library and per library type, the filtering
//! types Plex offers, the fields with their type and subtype, the operators
//! legal for each field type, and the enumerated choices with the fast key that
//! lists matching items directly.
//!
//! Reading it is what makes "is this predicate Plex-native?" a lookup rather
//! than an allowlist somebody has to keep current: a field a newer Plex adds
//! appears, one an older Plex lacks does not, and a definition referencing an
//! absent field falls back to local evaluation with a recorded reason.

mod choices;
mod vocabulary;

pub use choices::FilterChoice;
pub use vocabulary::{
    DiscoveredField, DiscoveredFilter, DiscoveredSort, FieldOperator, FieldType, FilteringType,
    Vocabulary,
};
