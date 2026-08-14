// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a manageable hub is, and which of its three visibility axes are set.

use serde::Deserialize;

use crate::{collections::record::StringOrNumber, libraries::RatingKey};

/// Plex's identifier for one manageable hub.
///
/// Not a rating key: a native hub has no rating key at all, and a collection
/// hub's identifier is a composite Plex composes. Two different identifier
/// spaces in one `String` is how a call ends up addressing the wrong row (P4).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HubIdentifier(String);

impl HubIdentifier {
    /// Wraps an identifier read back from storage or from an answer.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The identifier as text, for a path segment.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for HubIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What kind of participant a hub is.
///
/// The distinction §15.1 makes the whole placement algorithm out of: a
/// collection can leave the ordering space and come back with fresh spacing,
/// and a native hub is an anchor the plan works around.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubKind {
    /// A collection promoted onto the home screen.
    Collection,
    /// One of Plex's own rows — recently added, continue watching, and so on.
    ///
    /// Cannot be unpromoted, so the recovery move available to everything else
    /// does not exist for it.
    Native,
}

/// The three independent visibility axes of one hub (§15.5).
///
/// Three booleans and not one, because they are three surfaces: a hub can be on
/// the owner's home screen, on shared users' home screens, and on the library's
/// recommended row in any combination, and collapsing them loses the case an
/// operator most often wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HubVisibility {
    /// Visible on the owner's home screen.
    pub own_home: bool,
    /// Visible on shared users' home screens.
    pub shared_home: bool,
    /// Visible on the library's recommended row.
    pub recommended: bool,
}

impl HubVisibility {
    /// Whether the hub appears anywhere at all.
    #[must_use]
    pub const fn is_hidden(self) -> bool {
        !self.own_home && !self.shared_home && !self.recommended
    }

    /// The query pairs that write these three axes.
    #[must_use]
    pub fn pairs(self) -> Vec<(String, String)> {
        vec![
            (
                "promotedToOwnHome".to_owned(),
                i32::from(self.own_home).to_string(),
            ),
            (
                "promotedToSharedHome".to_owned(),
                i32::from(self.shared_home).to_string(),
            ),
            (
                "promotedToRecommended".to_owned(),
                i32::from(self.recommended).to_string(),
            ),
        ]
    }
}

/// One hub in a library's manageable ordering space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedHub {
    /// Plex's identifier for the hub.
    pub identifier: HubIdentifier,
    /// The title Plex shows on the row.
    pub title: String,
    /// Whether it is a collection or one of Plex's own rows.
    pub kind: HubKind,
    /// The collection behind it, for a collection hub.
    pub rating_key: Option<RatingKey>,
    /// Where the three visibility axes stand.
    pub visibility: HubVisibility,
}

/// A hub exactly as Plex's JSON carries it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HubBody {
    #[serde(default)]
    hub_identifier: Option<String>,
    #[serde(default)]
    identifier: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    rating_key: Option<StringOrNumber>,
    #[serde(default)]
    promoted_to_own_home: Option<StringOrNumber>,
    #[serde(default)]
    promoted_to_shared_home: Option<StringOrNumber>,
    #[serde(default)]
    promoted_to_recommended: Option<StringOrNumber>,
}

/// Whether a flag Plex spells either way is set.
fn flag(value: Option<StringOrNumber>) -> bool {
    value
        .and_then(|value| value.as_i64())
        .is_some_and(|value| value != 0)
}

impl TryFrom<HubBody> for ManagedHub {
    type Error = ();

    /// Builds a hub, or nothing when the answer named no hub.
    ///
    /// A row with no identifier cannot be moved or hidden, and inventing one
    /// would address a different row. Dropped rather than defaulted, and the
    /// caller counts what it dropped.
    fn try_from(body: HubBody) -> Result<Self, Self::Error> {
        // Emptiness is checked on each spelling before falling back, not on the
        // winner: a server that sends `hubIdentifier: ""` alongside a usable
        // `identifier` would otherwise have the empty one shadow the fallback,
        // and the row would be dropped as unaddressable while it was addressable
        // all along.
        let identifier = body
            .hub_identifier
            .filter(|value| !value.is_empty())
            .or_else(|| body.identifier.filter(|value| !value.is_empty()))
            .ok_or(())?;
        let rating_key = body
            .rating_key
            .and_then(|value| value.as_i64())
            .map(|value| RatingKey::new(value.to_string()));
        Ok(Self {
            identifier: HubIdentifier::new(identifier),
            title: body.title.unwrap_or_default(),
            // A hub with a rating key is a collection; one without is Plex's
            // own row. Read from the answer rather than from the identifier's
            // spelling, which changes between versions.
            kind: if rating_key.is_some() {
                HubKind::Collection
            } else {
                HubKind::Native
            },
            rating_key,
            visibility: HubVisibility {
                own_home: flag(body.promoted_to_own_home),
                shared_home: flag(body.promoted_to_shared_home),
                recommended: flag(body.promoted_to_recommended),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hub(json: &str) -> Result<ManagedHub, ()> {
        let body: HubBody = serde_json::from_str(json).expect("parses");
        ManagedHub::try_from(body)
    }

    #[test]
    fn a_collection_hub_carries_its_rating_key() {
        let hub = hub(
            r#"{"hubIdentifier":"collection.5001","title":"Best of 1979","ratingKey":"5001",
                "promotedToOwnHome":"1","promotedToRecommended":"1"}"#,
        )
        .expect("a hub with an identifier");
        assert_eq!(hub.kind, HubKind::Collection);
        assert_eq!(hub.rating_key, Some(RatingKey::new("5001")));
        assert!(hub.visibility.own_home);
        assert!(!hub.visibility.shared_home);
        assert!(hub.visibility.recommended);
    }

    #[test]
    fn a_native_hub_has_no_rating_key_and_is_an_anchor() {
        // The distinction the whole placement algorithm is built on: this row
        // cannot be unpromoted, so it has no recovery move (§15.1).
        let hub = hub(r#"{"hubIdentifier":"home.continue","title":"Continue Watching"}"#)
            .expect("a hub with an identifier");
        assert_eq!(hub.kind, HubKind::Native);
        assert_eq!(hub.rating_key, None);
    }

    #[test]
    fn a_row_with_no_identifier_is_dropped_rather_than_given_one() {
        assert!(hub(r#"{"title":"Nameless"}"#).is_err());
        assert!(hub(r#"{"hubIdentifier":"","title":"Nameless"}"#).is_err());
    }

    #[test]
    fn an_empty_primary_spelling_falls_back_rather_than_shadowing_the_other() {
        // A row that names itself under `identifier` and sends `hubIdentifier`
        // empty is addressable, and dropping it would take a movable row out of
        // the ordering space and count it as one this build cannot reach.
        let hub = hub(r#"{"hubIdentifier":"","identifier":"home.continue"}"#)
            .expect("the fallback names the row");
        assert_eq!(hub.identifier, HubIdentifier::new("home.continue"));
    }

    #[test]
    fn the_three_axes_write_out_as_three_separate_arguments() {
        let visibility = HubVisibility {
            own_home: true,
            shared_home: false,
            recommended: true,
        };
        assert_eq!(
            visibility.pairs(),
            vec![
                ("promotedToOwnHome".to_owned(), "1".to_owned()),
                ("promotedToSharedHome".to_owned(), "0".to_owned()),
                ("promotedToRecommended".to_owned(), "1".to_owned()),
            ]
        );
        assert!(!visibility.is_hidden());
        assert!(HubVisibility::default().is_hidden());
    }

    #[test]
    fn a_flag_spelled_as_a_number_reads_the_same_as_one_spelled_as_a_string() {
        let numeric = hub(r#"{"hubIdentifier":"h","promotedToOwnHome":1}"#)
            .expect("a hub with an identifier");
        let text = hub(r#"{"hubIdentifier":"h","promotedToOwnHome":"1"}"#)
            .expect("a hub with an identifier");
        assert_eq!(numeric.visibility, text.visibility);
    }
}
