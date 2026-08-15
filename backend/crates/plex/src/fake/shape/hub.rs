// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One row of the manageable ordering space.

use crate::fake::{element::Element, state::FakeHub};

/// One row, in the shape `/hubs/sections/{key}/manage` answers with.
///
/// Named `identifier` (`plexapi/library.py:3037`), and never `hubIdentifier` —
/// that attribute belongs to `/hubs/sections/{key}`, which is a different call
/// answering a different class (`plexapi/library.py:725`, `:2226`). Emitting
/// both would be the fake covering for a client that reads either, which is
/// exactly the drift a reference client exists to catch.
///
/// No `ratingKey` either. `ManagedHub` never reads one
/// (`plexapi/library.py:3033-3046`), so this build has no evidence a real
/// server sends one here; what it sends instead is `deletable`, and that is
/// how a row says whether it can leave the space at all (§15.1).
pub(crate) fn hub(hub: &FakeHub) -> Element {
    Element::named("Hub")
        .text("identifier", hub.identifier.clone())
        .text("title", hub.title.clone())
        .text("type", "mixed")
        .flag("deletable", hub.deletable)
        .text("homeVisibility", hub.home_visibility())
        .text(
            "recommendationsVisibility",
            hub.recommendations_visibility(),
        )
        .flag("promotedToOwnHome", hub.own_home)
        .flag("promotedToSharedHome", hub.shared_home)
        .flag("promotedToRecommended", hub.recommended)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::json;

    /// A collection row: removable, on the owner's home and recommended.
    fn row() -> FakeHub {
        FakeHub {
            identifier: "custom.collection.1.15001".to_owned(),
            title: "Movies Collection".to_owned(),
            rating_key: Some("15001".to_owned()),
            deletable: true,
            own_home: true,
            shared_home: false,
            recommended: true,
        }
    }

    fn rendered(hub_row: &FakeHub) -> serde_json::Value {
        json::document(&hub(hub_row))
    }

    #[test]
    fn a_row_names_itself_the_way_the_manage_endpoint_does() {
        let body = rendered(&row());
        assert_eq!(body["Hub"]["identifier"], "custom.collection.1.15001");
        assert!(
            body["Hub"].get("hubIdentifier").is_none(),
            "that attribute belongs to a different endpoint"
        );
        assert!(
            body["Hub"].get("ratingKey").is_none(),
            "no reference client reads one here"
        );
    }

    #[test]
    fn one_of_plexs_own_rows_says_it_cannot_be_removed() {
        // The fact §15.1's anchor rests on, and the one the fake never sent.
        let anchor = FakeHub {
            deletable: false,
            ..row()
        };
        assert_eq!(rendered(&anchor)["Hub"]["deletable"], 0);
        assert_eq!(rendered(&row())["Hub"]["deletable"], 1);
    }

    #[test]
    fn the_home_visibility_word_follows_the_two_home_axes() {
        let word = |own_home, shared_home| {
            rendered(&FakeHub {
                own_home,
                shared_home,
                ..row()
            })["Hub"]["homeVisibility"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        };
        assert_eq!(word(true, true), "all");
        assert_eq!(word(true, false), "admin");
        assert_eq!(word(false, true), "shared");
        assert_eq!(word(false, false), "none");
    }

    #[test]
    fn the_three_axes_are_spelled_as_flags_and_not_as_strings() {
        let body = rendered(&row());
        assert_eq!(body["Hub"]["promotedToOwnHome"], 1);
        assert_eq!(body["Hub"]["promotedToSharedHome"], 0);
        assert_eq!(body["Hub"]["promotedToRecommended"], 1);
        assert_eq!(body["Hub"]["recommendationsVisibility"], "all");
    }
}
