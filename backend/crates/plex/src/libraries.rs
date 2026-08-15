// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Libraries, their items, and the query shapes that select them.
//!
//! [`LibrarySection`] is what the server has, [`LibraryItem`] is what is in
//! one, and [`ItemQuery`] carries the arguments Plex's own filter vocabulary is
//! expressed in — the operator suffixes (`!=`, `>>=`, `<<=`, `&=`, and the
//! doubled `=` for exact string matching) that PRD §13.2.4 names as the compile
//! target for a Plex-native filter tree.
//!
//! Nothing here decides which libraries Afisharr will manage. `music` and
//! `photo` sections are reported exactly as the server reports them; refusing
//! to represent them is the library cache's rule, not the protocol's.

mod filter;
mod item;
// `pub(crate)` for its response body alone: `crate::streams` reads one item
// through the same `Metadata` envelope this module parses a window with, and a
// second copy of that shape would be a second thing to keep in step (P7).
pub(crate) mod listing;
mod section;

pub use filter::{FilterArgument, FilterOperator, ItemQuery, Window};
pub use item::{ItemKind, LibraryItem, RatingKey, ScanState, SortTitle};
pub use listing::ItemPage;
pub use section::{LibraryKind, LibrarySection, SectionKey};
