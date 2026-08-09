// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Derived columns, and the sweep that proves they are still derived.
//!
//! Some tables store a canonical JSON body as the source of truth and extract a
//! few columns for indexing. Those columns are **derived**, and derived columns
//! obey one rule (PRD §19.1):
//!
//! > A derived column is written only by the projection function that reads the
//! > body. Nothing else ever assigns to it. Dropping every derived column and
//! > recomputing it from the bodies must be a no-op.
//!
//! Enforced three ways: one projection function per table, [`reproject`] which
//! recomputes all of them, and a test asserting that for every row in the
//! database `project(body_json)` equals the stored columns. Without that test,
//! "hot columns for indexing only" degrades into a second source of truth
//! within about two releases.

mod definitions;
mod error;
mod library_item_state;
mod reproject;

pub use definitions::{DefinitionColumns, project_definition};
pub use error::ProjectionError;
pub use library_item_state::{LifecycleAxes, StateInputs, project_state_hash};
pub use reproject::{Reprojection, reproject};
