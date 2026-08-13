// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `GET /library/sections` — what the server holds.

use afisharr_sources::outbound::Method;
use serde::Deserialize;

use crate::server::{PlexServerClient, ServerError};

/// A Plex section key.
///
/// Plex assigns it and Plex may change it, which makes it a binding rather than
/// an identity (P4). It is a newtype so it cannot be passed where a rating key
/// or a machine identifier is expected.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SectionKey(String);

impl SectionKey {
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

impl std::fmt::Display for SectionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What kind of library a section is.
///
/// `Other` carries the raw value rather than dropping it: a section type this
/// build has never heard of is a fact worth reporting to the operator, and a
/// type silently mapped onto one of the four would be a library Afisharr
/// managed by accident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryKind {
    /// A movie library.
    Movie,
    /// A TV library.
    Show,
    /// A music library. Representable here, never managed (PRD §19.7).
    Artist,
    /// A photo library. Representable here, never managed.
    Photo,
    /// Something this build does not recognise.
    Other(String),
}

impl LibraryKind {
    /// Reads the value Plex reports in `type`.
    #[must_use]
    pub fn from_plex(value: &str) -> Self {
        match value {
            "movie" => Self::Movie,
            "show" => Self::Show,
            "artist" => Self::Artist,
            "photo" => Self::Photo,
            other => Self::Other(other.to_owned()),
        }
    }

    /// The value as Plex spells it.
    #[must_use]
    pub fn as_plex(&self) -> &str {
        match self {
            Self::Movie => "movie",
            Self::Show => "show",
            Self::Artist => "artist",
            Self::Photo => "photo",
            Self::Other(raw) => raw,
        }
    }
}

/// One library on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibrarySection {
    /// Plex's section key. Changes; matched on `uuid` first (PRD §19.7).
    pub key: SectionKey,
    /// Plex's section uuid, stable across a key change when reported.
    pub uuid: Option<String>,
    /// What kind of library it is.
    pub kind: LibraryKind,
    /// The title the operator gave it.
    pub title: String,
    /// The metadata agent, when reported.
    pub agent: Option<String>,
    /// The library's language, when reported.
    pub language: Option<String>,
    /// When Plex last finished scanning it, in epoch seconds.
    pub scanned_at: Option<i64>,
    /// Whether Plex is scanning it right now.
    ///
    /// A pass that read an item count while this is true read a count that is
    /// still moving, which is the difference between "there are 40 items" and
    /// "40 items have been indexed so far" (P1).
    pub refreshing: bool,
}

/// The directory list `GET /library/sections` answers with.
#[derive(Debug, Deserialize)]
struct SectionsBody {
    #[serde(default, rename = "Directory")]
    directory: Vec<SectionBody>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SectionBody {
    key: String,
    #[serde(default)]
    uuid: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    title: String,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    scanned_at: Option<i64>,
    #[serde(default)]
    refreshing: bool,
}

impl From<SectionBody> for LibrarySection {
    fn from(body: SectionBody) -> Self {
        Self {
            key: SectionKey::new(body.key),
            uuid: body.uuid.filter(|value| !value.is_empty()),
            kind: LibraryKind::from_plex(&body.kind),
            title: body.title,
            agent: body.agent.filter(|value| !value.is_empty()),
            language: body.language.filter(|value| !value.is_empty()),
            scanned_at: body.scanned_at,
            refreshing: body.refreshing,
        }
    }
}

impl PlexServerClient {
    /// Lists every library on the server.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer or
    /// refused.
    #[tracing::instrument(skip(self))]
    pub async fn sections(&self) -> Result<Vec<LibrarySection>, ServerError> {
        let url = self.endpoint("library/sections", &[])?;
        let body: SectionsBody = self.container(Method::GET, &url, None).await?;
        Ok(body
            .directory
            .into_iter()
            .map(LibrarySection::from)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "Directory": [
        {"key":"1","uuid":"sec-uuid-1","type":"movie","title":"Movies",
         "agent":"tv.plex.agents.movie","language":"en-US","scannedAt":1758000000},
        {"key":"2","type":"show","title":"TV","refreshing":true},
        {"key":"3","type":"artist","title":"Music"},
        {"key":"9","type":"holograms","title":"Something New"}
      ]
    }"#;

    fn sections() -> Vec<LibrarySection> {
        let body: SectionsBody = serde_json::from_str(FIXTURE).expect("parses");
        body.directory
            .into_iter()
            .map(LibrarySection::from)
            .collect()
    }

    #[test]
    fn a_movie_section_reads_every_field_the_cache_needs() {
        let movies = &sections()[0];
        assert_eq!(movies.key, SectionKey::new("1"));
        assert_eq!(movies.uuid.as_deref(), Some("sec-uuid-1"));
        assert_eq!(movies.kind, LibraryKind::Movie);
        assert_eq!(movies.agent.as_deref(), Some("tv.plex.agents.movie"));
        assert_eq!(movies.scanned_at, Some(1_758_000_000));
        assert!(!movies.refreshing);
    }

    #[test]
    fn a_section_with_no_uuid_reports_absence_rather_than_an_empty_string() {
        // `libraries.section_uuid` carries a unique index over non-null values.
        // An empty string is a value, and two of them collide.
        assert_eq!(sections()[1].uuid, None);
    }

    #[test]
    fn a_scanning_section_says_so() {
        assert!(sections()[1].refreshing);
    }

    #[test]
    fn a_music_library_is_reported_rather_than_hidden_by_the_protocol() {
        // Refusing to represent it is the cache's rule (PRD §19.7). A client
        // that dropped it here would leave the operator's "why is my music
        // library missing" question unanswerable from the doctor page.
        assert_eq!(sections()[2].kind, LibraryKind::Artist);
    }

    #[test]
    fn a_type_this_build_has_never_seen_keeps_its_name() {
        assert_eq!(
            sections()[3].kind,
            LibraryKind::Other("holograms".to_owned())
        );
        assert_eq!(sections()[3].kind.as_plex(), "holograms");
    }

    #[test]
    fn every_known_kind_round_trips_through_plexs_spelling() {
        for kind in [
            LibraryKind::Movie,
            LibraryKind::Show,
            LibraryKind::Artist,
            LibraryKind::Photo,
        ] {
            assert_eq!(LibraryKind::from_plex(kind.as_plex()), kind);
        }
    }
}
