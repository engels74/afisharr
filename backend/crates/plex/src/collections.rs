// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Collections: creating them, editing them, and moving items inside them.
//!
//! Plex models a collection as an item of type 18, which is why a rating key
//! addresses one and why the edit call is the same `PUT` a movie's metadata
//! takes. The one shape that is genuinely collection-specific is the `uri=`
//! argument, which names the server by machine identifier — so creating a
//! collection needs the identity `I-ID-5` guards, and this module takes it as
//! an argument rather than assuming the client is bound to the right server.

mod crud;
mod items;
// `pub(crate)` for its body types alone: `crate::hubs` reads the same
// number-or-string spelling Plex uses for a flag, and a second copy of that
// tolerance would be a second thing to keep in step (P7).
pub(crate) mod record;
mod uri;

pub use crud::CollectionEdit;
pub use items::MoveTarget;
pub use record::{Collection, CollectionMode, CollectionSort};
pub use uri::library_uri;
