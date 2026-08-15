// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Creating a collection, editing one, and deleting one.

use afisharr_sources::outbound::Method;
use serde::Deserialize;

use crate::{
    collections::{
        Collection, CollectionMode, CollectionSort, library_uri, record::CollectionBody,
    },
    libraries::{ItemKind, RatingKey, SectionKey},
    server::{MachineIdentifier, PlexServerClient, ServerError},
};

/// The list a collection call answers with.
#[derive(Debug, Deserialize)]
struct CollectionsBody {
    #[serde(default, rename = "Metadata")]
    metadata: Vec<CollectionBody>,
    #[serde(default, rename = "Directory")]
    directory: Vec<CollectionBody>,
}

impl CollectionsBody {
    /// The rows, from whichever key this server's version used.
    ///
    /// A collection is a `Directory` element in XML and a `Metadata` entry in
    /// the JSON translation of the same answer. Which key a given server and a
    /// given endpoint use is a fact this repository has no evidence for, and
    /// the release-lane capture is what settles it; the earlier claim that the
    /// two spellings were a server-version difference was not evidence, it was
    /// a guess. Reading only one and finding nothing reports the library as
    /// having no collections at all — the empty-versus-unobserved conflation `I-SRC-1` forbids.
    fn rows(self) -> Vec<CollectionBody> {
        if self.metadata.is_empty() {
            self.directory
        } else {
            self.metadata
        }
    }
}

/// A change to a collection's editable fields.
///
/// Every field is optional and an omitted one is left alone. Plex's edit
/// endpoint writes only the arguments it is given, so a struct of plain values
/// would rewrite a title the operator changed in Plex a moment ago with the one
/// this process read a minute before (P2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollectionEdit {
    /// A new title.
    pub title: Option<String>,
    /// A new sort title, with the lock state to write alongside it.
    ///
    /// The lock travels with the value because Plex's edit endpoint writes both
    /// in one request, and a restore that left the field locked has disabled
    /// the server's own metadata refresh for that item, silently (`I-REV-3`).
    pub sort_title: Option<(String, bool)>,
    /// A new summary.
    pub summary: Option<String>,
    /// A new display mode.
    pub mode: Option<CollectionMode>,
    /// A new item order.
    pub sort: Option<CollectionSort>,
}

impl CollectionEdit {
    /// Whether this edit would write anything at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.sort_title.is_none()
            && self.summary.is_none()
            && self.mode.is_none()
            && self.sort.is_none()
    }

    /// The query pairs this edit contributes, in a stable order.
    #[must_use]
    pub fn pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::with_capacity(6);
        if let Some(title) = &self.title {
            pairs.push(("title.value".to_owned(), title.clone()));
        }
        if let Some((value, locked)) = &self.sort_title {
            pairs.push(("titleSort.value".to_owned(), value.clone()));
            pairs.push((
                "titleSort.locked".to_owned(),
                i32::from(*locked).to_string(),
            ));
        }
        if let Some(summary) = &self.summary {
            pairs.push(("summary.value".to_owned(), summary.clone()));
        }
        if let Some(mode) = self.mode {
            pairs.push(("collectionMode".to_owned(), mode.as_plex().to_string()));
        }
        if let Some(sort) = self.sort {
            pairs.push(("collectionSort".to_owned(), sort.as_plex().to_string()));
        }
        pairs
    }
}

impl PlexServerClient {
    /// Lists the collections in one library.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer.
    #[tracing::instrument(skip(self))]
    pub async fn collections(&self, section: &SectionKey) -> Result<Vec<Collection>, ServerError> {
        let url = self.endpoint(
            &format!("library/sections/{section}/collections"),
            &[("includeCollections".to_owned(), "1".to_owned())],
        )?;
        let body: CollectionsBody = self.container(Method::GET, &url, None).await?;
        Ok(body.rows().into_iter().map(Collection::from).collect())
    }

    /// Creates a collection holding `items`.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer, and
    /// [`ServerError::Incomplete`] when it answered without the collection it
    /// created — a created-but-unidentified collection cannot be adopted, and
    /// guessing at its key would bind Afisharr to somebody else's object.
    #[tracing::instrument(skip(self, items))]
    pub async fn create_collection(
        &self,
        section: &SectionKey,
        libtype: ItemKind,
        title: &str,
        server: &MachineIdentifier,
        items: &[RatingKey],
    ) -> Result<Collection, ServerError> {
        let uri = library_uri(server, items).ok_or(ServerError::Incomplete {
            call: "POST /library/collections",
            missing: "any item to put in it",
        })?;
        let url = self.endpoint(
            "library/collections",
            &[
                ("type".to_owned(), libtype.as_plex_type().to_string()),
                ("title".to_owned(), title.to_owned()),
                ("smart".to_owned(), "0".to_owned()),
                ("sectionId".to_owned(), section.to_string()),
                ("uri".to_owned(), uri),
            ],
        )?;
        let body: CollectionsBody = self.container(Method::POST, &url, None).await?;
        body.rows()
            .into_iter()
            .next()
            .map(Collection::from)
            .ok_or(ServerError::Incomplete {
                call: "POST /library/collections",
                missing: "the collection it created",
            })
    }

    /// Applies an edit to one collection.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer, and
    /// [`ServerError::Incomplete`] when the edit would write nothing — a call
    /// that sent no arguments is a `PUT` with no effect, and reporting it as a
    /// success is how a settings page claims to have saved something it did not.
    #[tracing::instrument(skip(self))]
    pub async fn edit_collection(
        &self,
        section: &SectionKey,
        collection: &RatingKey,
        edit: &CollectionEdit,
    ) -> Result<(), ServerError> {
        if edit.is_empty() {
            return Err(ServerError::Incomplete {
                call: "PUT /library/sections/{id}/all",
                missing: "any field to change",
            });
        }
        let mut query = vec![
            (
                "type".to_owned(),
                ItemKind::Collection.as_plex_type().to_string(),
            ),
            ("id".to_owned(), collection.to_string()),
        ];
        query.extend(edit.pairs());
        let url = self.endpoint(&format!("library/sections/{section}/all"), &query)?;
        self.send(Method::PUT, &url, None, &[]).await?;
        Ok(())
    }

    /// Deletes one collection.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer.
    #[tracing::instrument(skip(self))]
    pub async fn delete_collection(&self, collection: &RatingKey) -> Result<(), ServerError> {
        let url = self.endpoint(&format!("library/collections/{collection}"), &[])?;
        self.send(Method::DELETE, &url, None, &[]).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_collection_list_reads_the_current_key_and_the_older_one() {
        // Reading only `Metadata` reports an older server's library as having
        // no collections at all, which is `I-SRC-1`'s conflation exactly.
        let current: CollectionsBody =
            serde_json::from_str(r#"{"Metadata":[{"ratingKey":"1","title":"A"}]}"#)
                .expect("parses");
        let older: CollectionsBody =
            serde_json::from_str(r#"{"Directory":[{"ratingKey":"1","title":"A"}]}"#)
                .expect("parses");
        assert_eq!(current.rows().len(), 1);
        assert_eq!(older.rows().len(), 1);
    }

    #[test]
    fn an_edit_writes_only_the_fields_it_names() {
        let edit = CollectionEdit {
            title: Some("Best of 1979".to_owned()),
            ..CollectionEdit::default()
        };
        assert_eq!(
            edit.pairs(),
            vec![("title.value".to_owned(), "Best of 1979".to_owned())]
        );
    }

    #[test]
    fn a_sort_title_edit_carries_its_lock_state_in_the_same_request() {
        // Plex writes the lock alongside the value. Sent without it, a restore
        // leaves the field locked and the server's metadata refresh disabled
        // for that item, with no report (`I-REV-3`).
        let edit = CollectionEdit {
            sort_title: Some(("!001 Best".to_owned(), false)),
            ..CollectionEdit::default()
        };
        assert_eq!(
            edit.pairs(),
            vec![
                ("titleSort.value".to_owned(), "!001 Best".to_owned()),
                ("titleSort.locked".to_owned(), "0".to_owned()),
            ]
        );
    }

    #[test]
    fn an_edit_that_changes_nothing_is_recognisable_before_it_is_sent() {
        assert!(CollectionEdit::default().is_empty());
        assert!(CollectionEdit::default().pairs().is_empty());
    }

    #[test]
    fn the_mode_and_sort_go_out_as_the_numbers_plex_takes() {
        let edit = CollectionEdit {
            mode: Some(CollectionMode::HideItems),
            sort: Some(CollectionSort::Custom),
            ..CollectionEdit::default()
        };
        assert_eq!(
            edit.pairs(),
            vec![
                ("collectionMode".to_owned(), "1".to_owned()),
                ("collectionSort".to_owned(), "2".to_owned()),
            ]
        );
    }
}
