// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One library, in the shape the section list answers with.

use crate::fake::{element::Element, state::FakeLibrary};

/// The metadata agent a library of this kind declares.
///
/// One agent for every section would have a TV library declaring the movie
/// agent, which is a fact no real server states and exactly the drift the
/// contract test exists to catch.
const fn agent(kind: &str) -> &'static str {
    match kind.as_bytes() {
        b"show" => "tv.plex.agents.series",
        b"artist" => "tv.plex.agents.music",
        b"photo" => "tv.plex.agents.photo",
        _ => "tv.plex.agents.movie",
    }
}

/// One library, in the shape the section list answers with.
pub(crate) fn section(library: &FakeLibrary) -> Element {
    Element::named("Directory")
        .text("key", library.key.clone())
        .text("uuid", library.uuid.clone())
        .text("type", library.kind.clone())
        .text("title", library.title.clone())
        .text("agent", agent(&library.kind))
        // The scanner, and the two timestamps a client reads off a section
        // (`plexapi/library.py:440-458`). A library with no `createdAt` is a
        // library that cannot be sorted by age, and nothing here sent one.
        .text("scanner", library.scanner.clone())
        .text("language", "en-US")
        .flag("filters", true)
        .flag("refreshing", false)
        .number("createdAt", 1_690_000_000_i64)
        .number("updatedAt", 1_700_000_000_i64)
        .number("scannedAt", 1_700_000_000_i64)
        .flag("allowSync", false)
        .children(library.locations.iter().enumerate().map(|(index, path)| {
            Element::named("Location")
                .number("id", i64::try_from(index + 1).unwrap_or(1))
                .text("path", path.clone())
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{json, library::World, scenario::Scenario};

    fn rendered(index: usize) -> serde_json::Value {
        let world = World::build(&Scenario::behaving(1));
        json::document(&section(&world.libraries[index]))
    }

    #[test]
    fn a_section_names_its_agent_scanner_and_folders() {
        let movies = rendered(0);
        assert_eq!(movies["Directory"]["agent"], "tv.plex.agents.movie");
        assert_eq!(movies["Directory"]["scanner"], "Plex Movie");
        assert_eq!(movies["Directory"]["Location"][0]["path"], "/data/movie");
    }

    #[test]
    fn a_tv_library_declares_the_tv_agent_rather_than_the_movie_one() {
        assert_eq!(rendered(1)["Directory"]["agent"], "tv.plex.agents.series");
        assert_eq!(rendered(1)["Directory"]["scanner"], "Plex TV Series");
    }

    #[test]
    fn a_section_carries_the_timestamps_a_client_reads_off_it() {
        let movies = rendered(0);
        assert!(movies["Directory"]["createdAt"].is_number());
        assert!(movies["Directory"]["updatedAt"].is_number());
    }
}
