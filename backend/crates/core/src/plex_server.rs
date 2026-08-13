// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The one server this installation is bound to.
//!
//! One row, and the identity every Plex-bound row in the database hangs off. A
//! changed `machine_identifier` means the operator pointed this installation at
//! a *different server*, which invalidates every rating key, every discovered
//! field, and every adoption here (PRD §19.7). That is never silently
//! reconciled: `I-ID-5` requires the observation to be recorded, everything
//! Plex-bound to be treated as suspect, and an explicit operator decision
//! between "this is a new server, rebind" and "restore a backup".
//!
//! So the write in [`RecordObservation`] refuses to move the identifier. The
//! rebind is a separate, explicitly-named decision, and a write that could
//! quietly perform it is a write that eventually will.

mod store;

pub use store::{Observed, PlexServer, RecordObservation, load};
