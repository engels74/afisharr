// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Artwork: what a poster reference is, and how one is uploaded.
//!
//! Plex answers `thumb` in several shapes, and the shapes change between
//! versions. `I-ID-2` requires an unrecognised one to be *recorded and carried*
//! rather than guessed at or fatal, so a reference is classified at the
//! boundary into three cases — a server-relative path, an absolute URL, and one
//! this build does not recognise — and the third keeps its raw text.

mod reference;
mod upload;

pub use reference::{ArtworkKind, ArtworkRef};
pub use upload::ArtworkUpload;
