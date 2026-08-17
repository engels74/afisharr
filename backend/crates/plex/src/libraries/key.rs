// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! How an item is addressed, and what kind of thing it is.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_maps_to_the_numeric_type_plexs_queries_take() {
        assert_eq!(ItemKind::Movie.as_plex_type(), 1);
        assert_eq!(ItemKind::Show.as_plex_type(), 2);
        assert_eq!(ItemKind::Season.as_plex_type(), 3);
        assert_eq!(ItemKind::Episode.as_plex_type(), 4);
        assert_eq!(ItemKind::Collection.as_plex_type(), 18);
    }

    #[test]
    fn every_kind_round_trips_through_the_word_plex_reports() {
        for (kind, word) in [
            (ItemKind::Movie, "movie"),
            (ItemKind::Show, "show"),
            (ItemKind::Season, "season"),
            (ItemKind::Episode, "episode"),
            (ItemKind::Collection, "collection"),
        ] {
            assert_eq!(ItemKind::from_plex(word), Some(kind));
        }
    }

    #[test]
    fn a_type_this_build_does_not_model_is_absent_rather_than_guessed() {
        assert_eq!(ItemKind::from_plex("track"), None);
    }

    #[test]
    fn a_key_is_the_text_it_arrived_as() {
        // Plex's identifier space is Plex's. A key parsed and re-rendered is a
        // key this build normalised, and normalising somebody else's opaque
        // identifier is how two rows come to mean one thing (P4).
        assert_eq!(RatingKey::new("0042").as_str(), "0042");
        assert_eq!(RatingKey::new("5001a").to_string(), "5001a");
    }
}
