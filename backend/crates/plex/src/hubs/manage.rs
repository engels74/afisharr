// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `GET/PUT /hubs/sections/{key}/manage` — reading and moving the rows.

use afisharr_sources::outbound::Method;
use serde::Deserialize;

use crate::{
    hubs::{HubIdentifier, HubVisibility, ManagedHub, record::HubBody},
    libraries::{RatingKey, SectionKey},
    server::{PlexServerClient, ServerError},
};

/// Where a hub is being moved to.
///
/// The same relative-only primitive collection items have, and the same reason
/// for naming the head of the sequence explicitly (§15.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubMove {
    /// To the front of the ordering space.
    ToFront,
    /// Immediately after another hub.
    After(HubIdentifier),
}

impl HubMove {
    /// The query pairs this move contributes.
    fn pairs(&self) -> Vec<(String, String)> {
        match self {
            Self::ToFront => Vec::new(),
            Self::After(identifier) => vec![("after".to_owned(), identifier.to_string())],
        }
    }
}

/// The hub list `GET /hubs/sections/{key}/manage` answers with.
#[derive(Debug, Deserialize)]
struct HubsBody {
    #[serde(default, rename = "Hub")]
    hub: Vec<HubBody>,
}

/// Every hub in one answer, and how many rows named no hub.
///
/// The count is carried rather than dropped: a server answering ten rows of
/// which four have no identifier is a version difference worth a doctor-page
/// finding, and a silently shorter list looks exactly like a shorter home
/// screen (P1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubListing {
    /// The hubs this build can address.
    pub hubs: Vec<ManagedHub>,
    /// How many rows were dropped for naming no hub.
    pub unidentifiable: usize,
}

impl HubListing {
    /// The row for one collection, when the space already holds one.
    ///
    /// `None` is what a never-promoted collection looks like: it is in the
    /// library and not in the ordering space, and those are two states rather
    /// than one (`plexapi/collection.py:207-215`).
    #[must_use]
    pub fn row_for(&self, collection: &RatingKey) -> Option<&ManagedHub> {
        self.hubs
            .iter()
            .find(|hub| hub.names_collection(collection))
    }
}

impl PlexServerClient {
    /// Reads the manageable ordering space of one library.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer.
    #[tracing::instrument(skip(self))]
    pub async fn hubs(&self, section: &SectionKey) -> Result<HubListing, ServerError> {
        let url = self.endpoint(&format!("hubs/sections/{section}/manage"), &[])?;
        let body: HubsBody = self.container(Method::GET, &url, None).await?;
        Ok(listing(body))
    }

    /// Moves one hub within the ordering space.
    ///
    /// Reporting success is not evidence that the order changed. Past the
    /// precision budget Plex answers 200 and the item stays where it was, which
    /// is why every applied plan is verified by reading the order back (§15.3).
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer.
    #[tracing::instrument(skip(self))]
    pub async fn move_hub(
        &self,
        section: &SectionKey,
        hub: &HubIdentifier,
        target: &HubMove,
    ) -> Result<(), ServerError> {
        let url = self.endpoint(
            &format!("hubs/sections/{section}/manage/{hub}/move"),
            &target.pairs(),
        )?;
        self.send(Method::PUT, &url, None, &[]).await?;
        Ok(())
    }

    /// Writes the three visibility axes of one hub already in the space.
    ///
    /// Applied before ordering within a pass: an item has to be in the ordering
    /// space before its position can be set, and one being hidden should not
    /// spend a move (§15.5).
    ///
    /// This addresses an existing row. A collection nothing has promoted has no
    /// row to address, and that is what [`PlexServerClient::promote_hub`] is
    /// for — see [`PlexServerClient::set_collection_visibility`] for the choice
    /// between them.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer.
    #[tracing::instrument(skip(self))]
    pub async fn set_hub_visibility(
        &self,
        section: &SectionKey,
        hub: &HubIdentifier,
        visibility: HubVisibility,
    ) -> Result<(), ServerError> {
        let url = self.endpoint(
            &format!("hubs/sections/{section}/manage/{hub}"),
            &visibility.pairs(),
        )?;
        self.send(Method::PUT, &url, None, &[]).await?;
        Ok(())
    }

    /// Puts a collection into the ordering space, at the visibility given.
    ///
    /// The call a `PUT` cannot stand in for: until something promotes it, a
    /// collection has no row under `/manage/{identifier}` at all, and writing
    /// to one changes nothing (`plexapi/library.py:3114-3117`). The collection
    /// is named by `metadataItemId`, which is the rating key rather than a hub
    /// identifier — the space does not yet have one to give it.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer.
    #[tracing::instrument(skip(self))]
    pub async fn promote_hub(
        &self,
        section: &SectionKey,
        collection: &RatingKey,
        visibility: HubVisibility,
    ) -> Result<(), ServerError> {
        let mut query = vec![("metadataItemId".to_owned(), collection.to_string())];
        query.extend(visibility.pairs());
        let url = self.endpoint(&format!("hubs/sections/{section}/manage"), &query)?;
        self.send(Method::POST, &url, None, &[]).await?;
        Ok(())
    }

    /// Writes one collection's visibility, whichever state its row is in.
    ///
    /// The choice is made from the manage answer the caller already read, not
    /// guessed: a row that is there is written with a `PUT`, and one that is
    /// not is promoted with a `POST`. One implementation of the rule, so a
    /// caller cannot pick the wrong half (P7).
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer.
    #[tracing::instrument(skip(self, listing))]
    pub async fn set_collection_visibility(
        &self,
        section: &SectionKey,
        listing: &HubListing,
        collection: &RatingKey,
        visibility: HubVisibility,
    ) -> Result<(), ServerError> {
        match listing.row_for(collection) {
            Some(row) => {
                let identifier = row.identifier.clone();
                self.set_hub_visibility(section, &identifier, visibility)
                    .await
            }
            None => self.promote_hub(section, collection, visibility).await,
        }
    }
}

/// Turns an answer into a listing, counting what it could not address.
fn listing(body: HubsBody) -> HubListing {
    let total = body.hub.len();
    let hubs: Vec<ManagedHub> = body
        .hub
        .into_iter()
        .filter_map(|hub| ManagedHub::try_from(hub).ok())
        .collect();
    HubListing {
        unidentifiable: total - hubs.len(),
        hubs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "Hub": [
        {"identifier":"home.continue","title":"Continue Watching","deletable":"0",
         "promotedToOwnHome":"1"},
        {"identifier":"custom.collection.1.5001","title":"Best of 1979","deletable":"1",
         "promotedToOwnHome":"1","promotedToSharedHome":"1"},
        {"title":"A row this build cannot address"}
      ]
    }"#;

    fn parsed() -> HubListing {
        listing(serde_json::from_str(FIXTURE).expect("parses"))
    }

    #[test]
    fn the_ordering_space_reads_in_the_order_the_server_gave() {
        let listing = parsed();
        assert_eq!(listing.hubs[0].title, "Continue Watching");
        assert_eq!(listing.hubs[1].title, "Best of 1979");
    }

    #[test]
    fn a_row_this_build_cannot_address_is_counted_rather_than_vanishing() {
        // A silently shorter list is indistinguishable from a shorter home
        // screen, and the operator has no way to ask why (P1).
        assert_eq!(parsed().unidentifiable, 1);
        assert_eq!(parsed().hubs.len(), 2);
    }

    #[test]
    fn a_move_to_the_front_names_no_predecessor() {
        assert!(HubMove::ToFront.pairs().is_empty());
        assert_eq!(
            HubMove::After(HubIdentifier::new("home.continue")).pairs(),
            vec![("after".to_owned(), "home.continue".to_owned())]
        );
    }

    #[test]
    fn an_answer_with_no_hubs_is_an_empty_space_and_not_a_failure() {
        let listing = listing(serde_json::from_str(r#"{"size":0}"#).expect("parses"));
        assert!(listing.hubs.is_empty());
        assert_eq!(listing.unidentifiable, 0);
    }

    #[test]
    fn a_collection_with_a_row_is_found_and_one_without_is_absent() {
        // Absent is what a never-promoted collection looks like, and it is the
        // difference between a `PUT` that writes and a `PUT` that answers 200
        // to a row the server does not have.
        let listing = parsed();
        assert_eq!(
            listing
                .row_for(&RatingKey::new("5001"))
                .map(|row| row.identifier.clone()),
            Some(HubIdentifier::new("custom.collection.1.5001"))
        );
        assert!(listing.row_for(&RatingKey::new("6001")).is_none());
    }
}
