// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Media, parts, and streams — the file facts overlays are rendered from.
//!
//! Every field here is optional, and that is the point rather than defensive
//! typing: `media.*` is declared nullable in PRD §13.2.5, an item may have no
//! file at all, and Plex omits what it has not analysed. A `0` substituted for
//! an absent `audioChannels` is a badge that says "mono" about a file nobody
//! has looked at (P1).

mod facts;
mod media;

pub use media::{DolbyVision, MediaEntry, MediaPart, MediaStream, StreamKind};
