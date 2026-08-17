// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a collection is, as Plex reports it.

use serde::Deserialize;

use crate::{
    libraries::{RatingKey, SortTitle},
    wire::{Flag, StringOrNumber},
};

/// How Plex displays a collection in its library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionMode {
    /// Follow the library's own default.
    Default,
    /// Hide the collection, showing its items instead.
    HideCollection,
    /// Hide the items, showing only the collection.
    HideItems,
    /// Show both.
    ShowItems,
}

impl CollectionMode {
    /// The value Plex's `collectionMode` preference takes.
    #[must_use]
    pub const fn as_plex(self) -> i8 {
        match self {
            Self::Default => -1,
            Self::HideCollection => 0,
            Self::HideItems => 1,
            Self::ShowItems => 2,
        }
    }

    /// Reads the value back.
    #[must_use]
    pub const fn from_plex(value: i8) -> Option<Self> {
        match value {
            -1 => Some(Self::Default),
            0 => Some(Self::HideCollection),
            1 => Some(Self::HideItems),
            2 => Some(Self::ShowItems),
            _ => None,
        }
    }
}

/// How Plex orders the items inside a collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionSort {
    /// Release order.
    Release,
    /// Alphabetical.
    Alpha,
    /// The order the operator put them in, which is the one Afisharr plans.
    Custom,
}

impl CollectionSort {
    /// The value Plex's `collectionSort` preference takes.
    #[must_use]
    pub const fn as_plex(self) -> i8 {
        match self {
            Self::Release => 0,
            Self::Alpha => 1,
            Self::Custom => 2,
        }
    }

    /// Reads the value back.
    #[must_use]
    pub const fn from_plex(value: i8) -> Option<Self> {
        match value {
            0 => Some(Self::Release),
            1 => Some(Self::Alpha),
            2 => Some(Self::Custom),
            _ => None,
        }
    }
}

/// One collection on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    /// Plex's current key for it. A binding, not an identity (P4).
    pub rating_key: RatingKey,
    /// The collection's title.
    pub title: String,
    /// Its sort title, in all three properties §15.6 requires.
    pub sort_title: SortTitle,
    /// How many items Plex says are in it.
    pub child_count: Option<u32>,
    /// Whether Plex maintains it from a filter rather than from a list.
    pub smart: bool,
    /// The display mode, when the server reports one this build knows.
    pub mode: Option<CollectionMode>,
    /// The item order, when the server reports one this build knows.
    pub sort: Option<CollectionSort>,
}

/// A collection exactly as Plex's JSON carries it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollectionBody {
    pub(crate) rating_key: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    title_sort: Option<String>,
    #[serde(default)]
    child_count: Option<StringOrNumber>,
    #[serde(default)]
    smart: Flag,
    #[serde(default)]
    collection_mode: Option<StringOrNumber>,
    #[serde(default)]
    collection_sort: Option<StringOrNumber>,
    /// The fields Plex reports a metadata lock on.
    ///
    /// A collection row carries them exactly as an item row does — it is the
    /// same `Field` child on the same envelope. Read here because §15.6 wants
    /// all three properties of a sort title, and a lock dropped on the way in
    /// reads as *unlocked*: a teardown checking this would leave the
    /// operator's collection permanently locked and report that it had not
    /// (`I-REV-3`, P1).
    #[serde(default, rename = "Field")]
    field: Vec<FieldBody>,
}

/// One field's lock state, as Plex nests it.
#[derive(Debug, Deserialize)]
struct FieldBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    locked: Flag,
}

impl From<CollectionBody> for Collection {
    fn from(body: CollectionBody) -> Self {
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
            title: body.title.unwrap_or_default(),
            sort_title,
            child_count: body
                .child_count
                .and_then(|value| value.as_i64())
                .and_then(|value| u32::try_from(value).ok()),
            smart: body.smart.is_set(),
            mode: body
                .collection_mode
                .and_then(|value| value.as_i64())
                .and_then(|value| i8::try_from(value).ok())
                .and_then(CollectionMode::from_plex),
            sort: body
                .collection_sort
                .and_then(|value| value.as_i64())
                .and_then(|value| i8::try_from(value).ok())
                .and_then(CollectionSort::from_plex),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collection(json: &str) -> Collection {
        let body: CollectionBody = serde_json::from_str(json).expect("parses");
        Collection::from(body)
    }

    #[test]
    fn a_collection_reads_its_key_title_and_count() {
        let record = collection(
            r#"{"ratingKey":"5001","title":"Best of 1979","titleSort":"!001 Best of 1979",
                "childCount":"12","smart":"0","collectionSort":"2"}"#,
        );
        assert_eq!(record.rating_key, RatingKey::new("5001"));
        assert_eq!(record.child_count, Some(12));
        assert!(!record.smart);
        assert_eq!(record.sort, Some(CollectionSort::Custom));
        assert_eq!(record.sort_title.value(), Some("!001 Best of 1979"));
    }

    #[test]
    fn a_count_spelled_as_a_number_reads_the_same_as_one_spelled_as_a_string() {
        assert_eq!(
            collection(r#"{"ratingKey":"1","childCount":12}"#).child_count,
            collection(r#"{"ratingKey":"1","childCount":"12"}"#).child_count
        );
    }

    #[test]
    fn an_unreported_count_is_absent_rather_than_zero() {
        // Zero is a claim that the collection is empty, which is the fact
        // `I-SRC-1` refuses to synthesise from an answer that did not carry it.
        assert_eq!(collection(r#"{"ratingKey":"1"}"#).child_count, None);
    }

    #[test]
    fn a_locked_sort_title_is_read_off_the_collection_row_it_arrived_on() {
        // Dropped on the way in, a locked field read as unlocked — and a
        // teardown that checked this would leave the operator's collection
        // permanently locked and report that it had not (`I-REV-3`, P1). The
        // same `Field` child an item row carries, because it is the same
        // envelope.
        let record = collection(
            r#"{"ratingKey":"1","titleSort":"!001 Best",
                "Field":[{"name":"titleSort","locked":1}]}"#,
        );
        assert!(record.sort_title.is_locked());
        assert!(
            !collection(r#"{"ratingKey":"1","titleSort":"!001 Best"}"#)
                .sort_title
                .is_locked()
        );
    }

    #[test]
    fn a_sort_title_can_be_absent_and_locked_on_a_collection_too() {
        // The state a restore gets wrong, and the reason §15.6 names three
        // properties rather than one.
        let record = collection(r#"{"ratingKey":"1","Field":[{"name":"titleSort","locked":"1"}]}"#);
        assert!(!record.sort_title.is_present());
        assert!(record.sort_title.is_locked());
    }

    #[test]
    fn a_lock_on_another_field_is_not_a_lock_on_the_sort_title() {
        let record = collection(r#"{"ratingKey":"1","Field":[{"name":"label","locked":1}]}"#);
        assert!(!record.sort_title.is_locked());
    }

    #[test]
    fn a_smart_collection_says_so() {
        assert!(collection(r#"{"ratingKey":"1","smart":"1"}"#).smart);
        assert!(collection(r#"{"ratingKey":"1","smart":1}"#).smart);
    }

    #[test]
    fn every_mode_and_sort_round_trips_through_plexs_numbering() {
        for mode in [
            CollectionMode::Default,
            CollectionMode::HideCollection,
            CollectionMode::HideItems,
            CollectionMode::ShowItems,
        ] {
            assert_eq!(CollectionMode::from_plex(mode.as_plex()), Some(mode));
        }
        for sort in [
            CollectionSort::Release,
            CollectionSort::Alpha,
            CollectionSort::Custom,
        ] {
            assert_eq!(CollectionSort::from_plex(sort.as_plex()), Some(sort));
        }
    }

    #[test]
    fn a_mode_number_this_build_does_not_know_is_absent_rather_than_defaulted() {
        assert_eq!(CollectionMode::from_plex(7), None);
        assert_eq!(
            collection(r#"{"ratingKey":"1","collectionMode":7}"#).mode,
            None
        );
    }
}
