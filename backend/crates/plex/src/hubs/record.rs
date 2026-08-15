// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a manageable hub is, and which of its three visibility axes are set.

use serde::Deserialize;

use crate::{
    libraries::RatingKey,
    wire::{Flag, StringOrNumber},
};

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
///
/// Read from `deletable`, which is the server's own statement that a row can
/// leave the space (`plexapi/library.py:3035`). It used to be read from the
/// presence of a `ratingKey`, and no reference client reads a rating key on
/// this endpoint at all — so a real server sending none would have had every
/// collection row classified as one of Plex's own, which is a collection whose
/// position is never fixed.
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
    /// The collection behind it, when the server named one.
    ///
    /// A real server was never observed to send one here, so this is `None` far
    /// more often than not — [`ManagedHub::names_collection`] is how a row is
    /// matched to a collection, and it reads the identifier.
    pub rating_key: Option<RatingKey>,
    /// Where the three visibility axes stand.
    pub visibility: HubVisibility,
}

impl ManagedHub {
    /// Whether this row is the ordering-space row of `collection`.
    ///
    /// The last dot-segment of the identifier is the collection's rating key —
    /// the same reading a reference client makes when it promotes one
    /// (`plexapi/library.py:3115`) — and the answer's own `ratingKey` is used
    /// when a server sends it. Neither is invented: a row that names no
    /// collection either way is not that collection's row.
    #[must_use]
    pub fn names_collection(&self, collection: &RatingKey) -> bool {
        if self.rating_key.as_ref() == Some(collection) {
            return true;
        }
        self.identifier
            .as_str()
            .rsplit('.')
            .next()
            .is_some_and(|segment| segment == collection.as_str())
    }
}

/// A hub exactly as Plex's JSON carries it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HubBody {
    #[serde(default)]
    identifier: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    rating_key: Option<StringOrNumber>,
    /// Whether the row can leave the ordering space.
    ///
    /// Absent means it can, which is the reading a reference client makes
    /// (`plexapi/library.py:3035`). Defaulting the other way would classify
    /// every collection row on a server that omits the attribute as one of
    /// Plex's own, and take every one of them out of the plan.
    #[serde(default = "removable")]
    deletable: Flag,
    #[serde(default)]
    promoted_to_own_home: Flag,
    #[serde(default)]
    promoted_to_shared_home: Flag,
    #[serde(default)]
    promoted_to_recommended: Flag,
}

/// What `deletable` means when a server does not send it.
fn removable() -> Flag {
    Flag::from(true)
}

impl TryFrom<HubBody> for ManagedHub {
    type Error = ();

    /// Builds a hub, or nothing when the answer named no hub.
    ///
    /// A row with no identifier cannot be moved or hidden, and inventing one
    /// would address a different row. Dropped rather than defaulted, and the
    /// caller counts what it dropped.
    fn try_from(body: HubBody) -> Result<Self, Self::Error> {
        let identifier = body
            .identifier
            .filter(|value| !value.is_empty())
            .ok_or(())?;
        // Kept as the text it arrived as, never parsed and re-rendered. A key
        // this build could not read as a number would otherwise come back as
        // `None`, and `None` here is not "no key" — it is what makes the row one
        // of Plex's own, which cannot be unpromoted and which the placement
        // algorithm plans around as an anchor (§15.1). A collection silently
        // demoted to an anchor is a collection whose position is never fixed.
        let rating_key = body
            .rating_key
            .map(StringOrNumber::into_text)
            .filter(|value| !value.is_empty())
            .map(RatingKey::new);
        Ok(Self {
            identifier: HubIdentifier::new(identifier),
            title: body.title.unwrap_or_default(),
            // A row that says it cannot be removed is one of Plex's own, and a
            // row that can be is a promoted collection. The server states it;
            // nothing here infers it from a field the server may not send.
            kind: if body.deletable.is_set() {
                HubKind::Collection
            } else {
                HubKind::Native
            },
            rating_key,
            visibility: HubVisibility {
                own_home: body.promoted_to_own_home.is_set(),
                shared_home: body.promoted_to_shared_home.is_set(),
                recommended: body.promoted_to_recommended.is_set(),
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
    fn a_row_that_can_leave_the_space_is_a_collection() {
        let hub = hub(
            r#"{"identifier":"custom.collection.1.5001","title":"Best of 1979",
                "deletable":"1","promotedToOwnHome":"1","promotedToRecommended":"1"}"#,
        )
        .expect("a hub with an identifier");
        assert_eq!(hub.kind, HubKind::Collection);
        assert!(hub.visibility.own_home);
        assert!(!hub.visibility.shared_home);
        assert!(hub.visibility.recommended);
        assert!(
            hub.names_collection(&RatingKey::new("5001")),
            "the last segment of the identifier is the collection"
        );
        assert!(!hub.names_collection(&RatingKey::new("5002")));
    }

    #[test]
    fn a_row_that_cannot_be_removed_is_one_of_plexs_own_and_an_anchor() {
        // The distinction the whole placement algorithm is built on: this row
        // cannot be unpromoted, so it has no recovery move (§15.1). It is the
        // server's own statement, not an inference from a missing rating key —
        // no reference client reads a rating key on this endpoint at all.
        let hub =
            hub(r#"{"identifier":"home.continue","title":"Continue Watching","deletable":"0"}"#)
                .expect("a hub with an identifier");
        assert_eq!(hub.kind, HubKind::Native);
    }

    #[test]
    fn a_row_that_says_nothing_about_removal_is_treated_as_removable() {
        // The reading a reference client makes (`plexapi/library.py:3035`).
        // Defaulting the other way would take every collection row on a server
        // that omits the attribute out of the plan.
        let hub = hub(r#"{"identifier":"custom.collection.1.5001"}"#).expect("a hub");
        assert_eq!(hub.kind, HubKind::Collection);
    }

    #[test]
    fn a_row_with_no_identifier_is_dropped_rather_than_given_one() {
        assert!(hub(r#"{"title":"Nameless"}"#).is_err());
        assert!(hub(r#"{"identifier":"","title":"Nameless"}"#).is_err());
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
    fn a_rating_key_keeps_the_text_it_arrived_as_when_a_server_sends_one() {
        // Plex's identifier space is Plex's. A key parsed and re-rendered is a
        // key this build normalised. No reference client reads one here, so
        // this is tolerance for a server that turns out to send it — never a
        // fact the classification depends on.
        let numeric = hub(r#"{"identifier":"h","ratingKey":5001}"#).expect("a hub");
        let text = hub(r#"{"identifier":"h","ratingKey":"5001"}"#).expect("a hub");
        assert_eq!(numeric.rating_key, text.rating_key);
        assert_eq!(numeric.rating_key, Some(RatingKey::new("5001")));
        assert!(numeric.names_collection(&RatingKey::new("5001")));

        let odd = hub(r#"{"identifier":"h","ratingKey":"5001a"}"#).expect("a hub");
        assert_eq!(odd.rating_key, Some(RatingKey::new("5001a")));

        let empty = hub(r#"{"identifier":"h","ratingKey":""}"#).expect("a hub");
        assert_eq!(empty.rating_key, None);
    }

    #[test]
    fn a_flag_reads_the_same_in_every_spelling_a_server_uses() {
        let numeric = hub(r#"{"identifier":"h","promotedToOwnHome":1}"#).expect("a hub");
        let text = hub(r#"{"identifier":"h","promotedToOwnHome":"1"}"#).expect("a hub");
        let boolean = hub(r#"{"identifier":"h","promotedToOwnHome":true}"#).expect("a hub");
        assert_eq!(numeric.visibility, text.visibility);
        assert_eq!(numeric.visibility, boolean.visibility);
        assert!(numeric.visibility.own_home);
    }
}
