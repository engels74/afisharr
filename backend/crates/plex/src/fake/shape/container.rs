// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The envelope every Plex answer arrives in.

use crate::fake::{element::Element, state::FakeLibrary};

/// The envelope, with the attributes a client reads off every answer.
pub(crate) fn container() -> Element {
    Element::named("MediaContainer")
        .number("size", 0_i64)
        .flag("allowSync", false)
        .text("identifier", "com.plexapp.plugins.library")
        .text("mediaTagPrefix", "/system/bundle/media/flags/")
        .number("mediaTagVersion", 1_700_000_000_i64)
}

/// The envelope for an answer about one library's content.
///
/// The three `librarySection*` attributes are how a row learns which library it
/// came from (`plexapi/base.py:359-362`, `:1243-1245`), and a client copies
/// them onto every row it builds. Without the first of them a collection does
/// not know its own section, and asking for its ordering-space row requests
/// `/hubs/sections/None/manage` — a 404 that reads as "this collection is not
/// promoted".
pub(crate) fn library_container(library: &FakeLibrary) -> Element {
    container()
        .text("librarySectionID", library.key.clone())
        .text("librarySectionTitle", library.title.clone())
        .text("librarySectionUUID", library.uuid.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{json, scenario::Scenario};

    fn library() -> FakeLibrary {
        crate::fake::library::World::build(&Scenario::behaving(1))
            .libraries
            .swap_remove(0)
    }

    #[test]
    fn a_library_answer_names_the_section_it_came_from() {
        let rendered = json::document(&library_container(&library()));
        assert_eq!(rendered["MediaContainer"]["librarySectionID"], "1");
        assert_eq!(rendered["MediaContainer"]["librarySectionTitle"], "Movies");
        assert_eq!(
            rendered["MediaContainer"]["librarySectionUUID"],
            "uuid-section-1"
        );
    }

    #[test]
    fn an_answer_that_is_not_about_a_library_carries_no_section_at_all() {
        // `GET /identity` is not about a library, and a section id on it would
        // be a fact invented to fill a field.
        let rendered = json::document(&container());
        assert!(rendered["MediaContainer"].get("librarySectionID").is_none());
    }
}
