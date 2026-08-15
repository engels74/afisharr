// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What one library item is, as Plex reports it.

use serde::Deserialize;

use crate::{
    artwork::ArtworkRef,
    libraries::{ItemKind, RatingKey, SortTitle},
    streams::MediaEntry,
    wire::Flag,
};

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
    /// The external ids Plex reports alongside the primary guid.
    ///
    /// `imdb://tt0078748`, `tmdb://348`, and so on: the values external-id
    /// resolution matches on, which PRD section 21.2 calls the highest-volume
    /// lookup in the product. A separate list because they are separate facts,
    /// and the primary guid is Plex's own.
    pub external_guids: Vec<String>,
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
    /// Whether Plex is still analysing the item.
    ///
    /// Read through the permissive flag every neighbouring field goes through.
    /// Typed as a strict `bool`, a server sending `1` here failed the whole
    /// item parse — one attribute costing every other fact on the item.
    #[serde(default)]
    refreshing: Flag,
    #[serde(default)]
    thumb: Option<String>,
    #[serde(default, rename = "Media")]
    media: Vec<MediaEntry>,
    #[serde(default, rename = "Label")]
    label: Vec<TagBody>,
    #[serde(default, rename = "Field")]
    field: Vec<FieldBody>,
    #[serde(default, rename = "Guid")]
    guids: Vec<GuidBody>,
}

/// An external id as Plex nests it, `{"id": "imdb://tt0078748"}`.
#[derive(Debug, Deserialize)]
struct GuidBody {
    #[serde(default)]
    id: Option<String>,
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
    locked: Flag,
}

impl From<ItemBody> for LibraryItem {
    fn from(body: ItemBody) -> Self {
        let locked = body
            .field
            .iter()
            .any(|field| field.name.as_deref() == Some("titleSort") && field.locked.is_set());
        let sort_title = match body.title_sort {
            Some(value) => SortTitle::present(value, locked),
            None => SortTitle::absent(locked),
        };
        Self {
            rating_key: RatingKey::new(body.rating_key),
            guid: body.guid.filter(|value| !value.is_empty()),
            external_guids: body
                .guids
                .into_iter()
                .filter_map(|guid| guid.id)
                .filter(|id| !id.is_empty())
                .collect(),
            kind: ItemKind::from_plex(&body.kind),
            title: body.title.unwrap_or_default(),
            sort_title,
            year: body.year,
            index: body.index,
            parent_rating_key: body.parent_rating_key.map(RatingKey::new),
            originally_available_at: body.originally_available_at,
            added_at: body.added_at,
            updated_at: body.updated_at,
            scan: if body.refreshing.is_set() {
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
    fn a_flag_reads_the_same_in_every_spelling_a_server_uses() {
        // Both of these are XML attributes on the wire, and a strict `bool`
        // did not read the wrong value — it failed the whole item parse, and
        // took every other fact on the item with it.
        for spelling in ["1", "true", r#""1""#] {
            let movie = item(&format!(
                r#"{{"ratingKey":"1","type":"movie","refreshing":{spelling},
                    "Field":[{{"name":"titleSort","locked":{spelling}}}]}}"#
            ));
            assert_eq!(movie.scan, ScanState::Indexing, "{spelling}");
            assert!(movie.sort_title.is_locked(), "{spelling}");
        }
        for spelling in ["0", "false", r#""0""#] {
            let movie = item(&format!(
                r#"{{"ratingKey":"1","type":"movie","refreshing":{spelling}}}"#
            ));
            assert_eq!(movie.scan, ScanState::Complete, "{spelling}");
        }
    }

    #[test]
    fn the_facts_a_resolver_matches_on_are_read_off_the_answer() {
        let movie = item(
            r#"{"ratingKey":"1","type":"episode","title":"Alien","index":3,
                "parentRatingKey":"77","originallyAvailableAt":"1979-05-25"}"#,
        );
        assert_eq!(movie.index, Some(3));
        assert_eq!(movie.parent_rating_key, Some(RatingKey::new("77")));
        assert_eq!(movie.originally_available_at.as_deref(), Some("1979-05-25"));
    }

    #[test]
    fn the_external_ids_a_resolver_matches_on_are_read_off_the_answer() {
        // Parsed nowhere before this, and sent by the fake nowhere either, so
        // the highest-volume lookup in the product had no input at all.
        let movie = item(
            r#"{"ratingKey":"1","type":"movie",
                "Guid":[{"id":"imdb://tt0078748"},{"id":"tmdb://348"},{"id":""},{}]}"#,
        );
        assert_eq!(movie.external_guids, ["imdb://tt0078748", "tmdb://348"]);
    }
}
