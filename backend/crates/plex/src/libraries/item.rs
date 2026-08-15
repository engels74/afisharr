// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What one library item is, as Plex reports it.

use serde::Deserialize;

use crate::{artwork::ArtworkRef, streams::MediaEntry};

/// A Plex rating key.
///
/// Plex assigns it, and Plex changes it — a re-scan, a metadata refresh, or a
/// file move is enough. It is a *binding*, never an identity (P4), and it is a
/// newtype so it cannot be handed to a call expecting a section key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RatingKey(String);

impl RatingKey {
    /// Wraps a key read back from storage or from an answer.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The key as text, for a path segment or a query value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RatingKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What kind of thing an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// A film.
    Movie,
    /// A series.
    Show,
    /// A season of a series.
    Season,
    /// An episode.
    Episode,
    /// A collection, which Plex models as an item of its own.
    Collection,
}

impl ItemKind {
    /// The numeric `type` Plex's query parameters take.
    #[must_use]
    pub const fn as_plex_type(self) -> u8 {
        match self {
            Self::Movie => 1,
            Self::Show => 2,
            Self::Season => 3,
            Self::Episode => 4,
            Self::Collection => 18,
        }
    }

    /// Reads the value Plex reports in an item's `type` attribute.
    #[must_use]
    pub fn from_plex(value: &str) -> Option<Self> {
        match value {
            "movie" => Some(Self::Movie),
            "show" => Some(Self::Show),
            "season" => Some(Self::Season),
            "episode" => Some(Self::Episode),
            "collection" => Some(Self::Collection),
            _ => None,
        }
    }
}

/// Whether Plex has finished indexing an item.
///
/// The distinction `I-EVID-*` rests on. An item Plex is still analysing reports
/// no media, no duration, and no streams — none of which means the file has no
/// audio track. Treating an in-progress scan as a completed one is P1 in its
/// purest form, so the state is carried on the item and every media accessor
/// goes through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanState {
    /// Plex reports the item as indexed.
    Complete,
    /// Plex is still working on it. What it does not report is unknown, not
    /// absent.
    Indexing,
}

/// An item's sort title, in the three properties §15.6 requires.
///
/// Value, presence, and lock state are independent, and all three round-trip.
/// Presence is read from the raw attribute rather than from a parsed value
/// because Plex clients substitute the title for a missing sort title, which
/// makes "absent" and "equal to the title" indistinguishable afterwards — and a
/// teardown that restored the substituted value would write a sort title the
/// operator never had.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SortTitle {
    value: Option<String>,
    locked: bool,
}

impl SortTitle {
    /// A sort title with `value` present and the given lock state.
    #[must_use]
    pub fn present(value: impl Into<String>, locked: bool) -> Self {
        Self {
            value: Some(value.into()),
            locked,
        }
    }

    /// A sort title Plex did not report, with the given lock state.
    #[must_use]
    pub const fn absent(locked: bool) -> Self {
        Self {
            value: None,
            locked,
        }
    }

    /// The raw value, or `None` when the attribute was absent.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    /// Whether the attribute was present at all.
    #[must_use]
    pub const fn is_present(&self) -> bool {
        self.value.is_some()
    }

    /// Whether Plex's own metadata lock is set on the field.
    ///
    /// A restore that leaves the field locked has permanently disabled the
    /// server's metadata refresh for that item, silently (`I-REV-3`).
    #[must_use]
    pub const fn is_locked(&self) -> bool {
        self.locked
    }
}

/// One item in a library.
//
// `PartialEq` without `Eq`: a media entry carries an aspect ratio, which is a
// float, and a float has no reflexive equality to promise.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryItem {
    /// Plex's current key for it. A binding, not an identity.
    pub rating_key: RatingKey,
    /// The primary guid, when reported.
    pub guid: Option<String>,
    /// What kind of item it is, or `None` for a type this build does not model.
    pub kind: Option<ItemKind>,
    /// The title.
    pub title: String,
    /// The sort title, in all three of its properties.
    pub sort_title: SortTitle,
    /// The release year, when reported.
    pub year: Option<i32>,
    /// The season or episode number, when reported.
    pub index: Option<i32>,
    /// The parent's rating key — season for an episode, show for a season.
    pub parent_rating_key: Option<RatingKey>,
    /// The civil release date, as Plex spells it.
    pub originally_available_at: Option<String>,
    /// When Plex added it, in epoch seconds.
    pub added_at: Option<i64>,
    /// When Plex last updated it, in epoch seconds.
    pub updated_at: Option<i64>,
    /// Whether Plex has finished indexing it.
    pub scan: ScanState,
    /// The poster reference, classified rather than assumed (`I-ID-2`).
    pub thumb: Option<ArtworkRef>,
    /// The labels on it.
    pub labels: Vec<String>,
    media: Vec<MediaEntry>,
}

impl LibraryItem {
    /// The media Plex reports, or `None` while it is still indexing.
    ///
    /// The whole reason [`ScanState`] is on this type. An empty list from a
    /// completed scan means the item genuinely has no file; the same empty list
    /// mid-scan means nothing at all, and a lifecycle pass that read the second
    /// as the first would mark a title missing while Plex was still reading it.
    #[must_use]
    pub fn media(&self) -> Option<&[MediaEntry]> {
        match self.scan {
            ScanState::Complete => Some(&self.media),
            ScanState::Indexing => None,
        }
    }

    /// The media Plex reported, whatever the scan state.
    ///
    /// For the one caller that wants the raw answer — the doctor page, which
    /// reports what was seen rather than what it means.
    #[must_use]
    pub fn media_as_reported(&self) -> &[MediaEntry] {
        &self.media
    }
}

/// One item exactly as Plex's JSON carries it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ItemBody {
    pub(crate) rating_key: String,
    #[serde(default)]
    guid: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    title_sort: Option<String>,
    #[serde(default)]
    year: Option<i32>,
    #[serde(default)]
    index: Option<i32>,
    #[serde(default)]
    parent_rating_key: Option<String>,
    #[serde(default)]
    originally_available_at: Option<String>,
    #[serde(default)]
    added_at: Option<i64>,
    #[serde(default)]
    updated_at: Option<i64>,
    #[serde(default)]
    refreshing: bool,
    #[serde(default)]
    thumb: Option<String>,
    #[serde(default, rename = "Media")]
    media: Vec<MediaEntry>,
    #[serde(default, rename = "Label")]
    label: Vec<TagBody>,
    #[serde(default, rename = "Field")]
    field: Vec<FieldBody>,
}

/// A tag as Plex nests it — `{"tag": "4K"}`.
#[derive(Debug, Deserialize)]
pub(crate) struct TagBody {
    #[serde(default)]
    pub(crate) tag: Option<String>,
}

/// A locked-field marker — `{"name": "titleSort", "locked": true}`.
#[derive(Debug, Deserialize)]
struct FieldBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    locked: bool,
}

impl From<ItemBody> for LibraryItem {
    fn from(body: ItemBody) -> Self {
        let locked = body
            .field
            .iter()
            .any(|field| field.name.as_deref() == Some("titleSort") && field.locked);
        let sort_title = match body.title_sort {
            Some(value) => SortTitle::present(value, locked),
            None => SortTitle::absent(locked),
        };
        Self {
            rating_key: RatingKey::new(body.rating_key),
            guid: body.guid.filter(|value| !value.is_empty()),
            kind: ItemKind::from_plex(&body.kind),
            title: body.title.unwrap_or_default(),
            sort_title,
            year: body.year,
            index: body.index,
            parent_rating_key: body.parent_rating_key.map(RatingKey::new),
            originally_available_at: body.originally_available_at,
            added_at: body.added_at,
            updated_at: body.updated_at,
            scan: if body.refreshing {
                ScanState::Indexing
            } else {
                ScanState::Complete
            },
            thumb: body.thumb.as_deref().map(ArtworkRef::classify),
            labels: body
                .label
                .into_iter()
                .filter_map(|tag| tag.tag)
                .filter(|tag| !tag.is_empty())
                .collect(),
            media: body.media,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(json: &str) -> LibraryItem {
        let body: ItemBody = serde_json::from_str(json).expect("parses");
        LibraryItem::from(body)
    }

    #[test]
    fn a_movie_reads_the_fields_the_cache_tracks() {
        let movie = item(
            r#"{"ratingKey":"1001","guid":"plex://movie/5d77","type":"movie","title":"Alien",
                "titleSort":"Alien","year":1979,"addedAt":1700000000,"updatedAt":1700000100,
                "originallyAvailableAt":"1979-05-25","thumb":"/library/metadata/1001/thumb/17"}"#,
        );
        assert_eq!(movie.rating_key, RatingKey::new("1001"));
        assert_eq!(movie.kind, Some(ItemKind::Movie));
        assert_eq!(movie.year, Some(1979));
        assert_eq!(movie.originally_available_at.as_deref(), Some("1979-05-25"));
        assert!(movie.thumb.is_some());
    }

    #[test]
    fn an_absent_sort_title_stays_absent_rather_than_becoming_the_title() {
        // Plex clients substitute the title when parsing, and a teardown that
        // restored the substituted value would write a sort title the operator
        // never had (§15.6, `I-REV-3`).
        let movie = item(r#"{"ratingKey":"1","type":"movie","title":"Alien"}"#);
        assert!(!movie.sort_title.is_present());
        assert_eq!(movie.sort_title.value(), None);
        assert_ne!(movie.sort_title.value(), Some(movie.title.as_str()));
    }

    #[test]
    fn a_locked_sort_title_is_read_from_the_field_list() {
        let movie = item(
            r#"{"ratingKey":"1","type":"movie","title":"Alien","titleSort":"!001 Alien",
                "Field":[{"name":"titleSort","locked":true}]}"#,
        );
        assert!(movie.sort_title.is_locked());
        assert_eq!(movie.sort_title.value(), Some("!001 Alien"));
    }

    #[test]
    fn a_lock_on_another_field_is_not_a_lock_on_the_sort_title() {
        let movie = item(
            r#"{"ratingKey":"1","type":"movie","titleSort":"Alien",
                "Field":[{"name":"title","locked":true}]}"#,
        );
        assert!(!movie.sort_title.is_locked());
    }

    #[test]
    fn an_item_still_being_indexed_reports_no_media_facts_at_all() {
        // Not an empty list: an empty list is a claim that the file has no
        // streams, and mid-scan nothing is known either way (P1).
        let movie = item(r#"{"ratingKey":"1","type":"movie","refreshing":true}"#);
        assert_eq!(movie.scan, ScanState::Indexing);
        assert_eq!(movie.media(), None);
        assert!(movie.media_as_reported().is_empty());
    }

    #[test]
    fn a_completed_scan_with_no_media_is_a_fact() {
        let movie = item(r#"{"ratingKey":"1","type":"movie"}"#);
        assert_eq!(movie.scan, ScanState::Complete);
        assert_eq!(movie.media(), Some(&[][..]));
    }

    #[test]
    fn labels_are_read_and_empty_tags_are_dropped() {
        let movie =
            item(r#"{"ratingKey":"1","type":"movie","Label":[{"tag":"afisharr"},{"tag":""},{}]}"#);
        assert_eq!(movie.labels, vec!["afisharr".to_owned()]);
    }

    #[test]
    fn a_type_this_build_does_not_model_is_absent_rather_than_guessed() {
        assert_eq!(item(r#"{"ratingKey":"1","type":"track"}"#).kind, None);
    }

    #[test]
    fn every_kind_maps_to_the_numeric_type_plexs_queries_take() {
        assert_eq!(ItemKind::Movie.as_plex_type(), 1);
        assert_eq!(ItemKind::Show.as_plex_type(), 2);
        assert_eq!(ItemKind::Season.as_plex_type(), 3);
        assert_eq!(ItemKind::Episode.as_plex_type(), 4);
        assert_eq!(ItemKind::Collection.as_plex_type(), 18);
    }
}
