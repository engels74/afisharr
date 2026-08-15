// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Hubs: the rows on the home screen, and the order they sit in.
//!
//! Three kinds of thing share one ordering space (§15.1), and they are not
//! interchangeable: a managed or adopted collection can be unpromoted and
//! re-promoted, which is the only way to win back precision, and a native hub
//! cannot. [`HubKind`] carries that difference so a planner never emits the
//! recovery move for a participant that has none.

mod manage;
mod record;

pub use manage::{HubListing, HubMove};
pub use record::{HubIdentifier, HubKind, HubVisibility, ManagedHub};
