// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Adding, removing, and reordering the items inside a collection.

use afisharr_sources::outbound::Method;

use crate::{
    collections::library_uri,
    libraries::listing::ItemsBody,
    libraries::{ItemPage, ItemQuery, LibraryItem, RatingKey},
    server::{MachineIdentifier, PlexServerClient, ServerError},
};

/// Where an item is being moved to.
///
/// Plex's only ordering primitive is relative — put A after B — and the head of
/// the sequence has no predecessor, so it is its own case rather than an
/// `Option<RatingKey>` that every call site has to remember means "first"
/// (§15.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveTarget {
    /// To the front of the collection.
    ToFront,
    /// Immediately after another item.
    After(RatingKey),
}

impl MoveTarget {
    /// The query pairs this target contributes.
    fn pairs(&self) -> Vec<(String, String)> {
        match self {
            Self::ToFront => Vec::new(),
            Self::After(key) => vec![("after".to_owned(), key.to_string())],
        }
    }
}

impl PlexServerClient {
    /// Reads one window of a collection's items.
    ///
    /// The read half of the verification §15.3 requires: a move that reports
    /// success and did not happen is only visible by reading the order back.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer.
    #[tracing::instrument(skip(self, query))]
    pub async fn collection_items(
        &self,
        collection: &RatingKey,
        query: &ItemQuery,
    ) -> Result<ItemPage, ServerError> {
        let url = self.endpoint(
            &format!("library/collections/{collection}/children"),
            &query.pairs(),
        )?;
        let body: ItemsBody = self.container(Method::GET, &url, None).await?;
        Ok(ItemPage {
            items: body.metadata.into_iter().map(LibraryItem::from).collect(),
            total: body.total_size,
            window: query.window(),
        })
    }

    /// Adds items to a collection.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer, and
    /// [`ServerError::Incomplete`] for an empty item set — the URI that would
    /// name it addresses every item Plex can find.
    #[tracing::instrument(skip(self, items))]
    pub async fn add_collection_items(
        &self,
        collection: &RatingKey,
        server: &MachineIdentifier,
        items: &[RatingKey],
    ) -> Result<(), ServerError> {
        let uri = library_uri(server, items).ok_or(ServerError::Incomplete {
            call: "PUT /library/collections/{id}/items",
            missing: "any item to add",
        })?;
        let url = self.endpoint(
            &format!("library/collections/{collection}/items"),
            &[("uri".to_owned(), uri)],
        )?;
        self.send(Method::PUT, &url, None, &[]).await?;
        Ok(())
    }

    /// Removes one item from a collection.
    ///
    /// One item per call, and not a batch: Plex's removal endpoint addresses a
    /// single child, and a batch wrapper here would report partial success as
    /// success.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer.
    #[tracing::instrument(skip(self))]
    pub async fn remove_collection_item(
        &self,
        collection: &RatingKey,
        item: &RatingKey,
    ) -> Result<(), ServerError> {
        let url = self.endpoint(
            &format!("library/collections/{collection}/items/{item}"),
            &[],
        )?;
        self.send(Method::DELETE, &url, None, &[]).await?;
        Ok(())
    }

    /// Moves one item within a collection.
    ///
    /// The call reporting success does not mean the move happened: past the
    /// precision budget Plex answers 200 and leaves the order alone (§15.3).
    /// Verification is the caller's, by reading the order back, which is why
    /// this returns nothing rather than a new position it cannot know.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer.
    #[tracing::instrument(skip(self))]
    pub async fn move_collection_item(
        &self,
        collection: &RatingKey,
        item: &RatingKey,
        target: &MoveTarget,
    ) -> Result<(), ServerError> {
        let url = self.endpoint(
            &format!("library/collections/{collection}/items/{item}/move"),
            &target.pairs(),
        )?;
        self.send(Method::PUT, &url, None, &[]).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{ServerAddress, ServerToken};
    use afisharr_sources::outbound::OutboundClient;

    use crate::identity::ClientIdentity;

    fn client() -> PlexServerClient {
        PlexServerClient::new(
            OutboundClient::new("afisharr/test").expect("the transport must build"),
            ClientIdentity::new("01JABCDEF", "Living Room", "0.1.0").expect("a valid identity"),
            ServerAddress::parse("http://plex.lan:32400").expect("a valid address"),
            ServerToken::new("plex-token").expect("a header-safe token"),
        )
    }

    #[test]
    fn a_move_to_the_front_carries_no_predecessor() {
        // Not `after=`, and not `after=` with an empty value: the first is the
        // head of the sequence and the second is a request to follow an item
        // with no key, which Plex answers 200 to and ignores.
        assert!(MoveTarget::ToFront.pairs().is_empty());
    }

    #[test]
    fn a_move_after_an_item_names_it() {
        assert_eq!(
            MoveTarget::After(RatingKey::new("42")).pairs(),
            vec![("after".to_owned(), "42".to_owned())]
        );
    }

    #[test]
    fn the_move_endpoint_addresses_the_item_inside_the_collection() {
        let url = client()
            .endpoint(
                "library/collections/5001/items/1001/move",
                &MoveTarget::After(RatingKey::new("999")).pairs(),
            )
            .expect("a valid endpoint");
        assert_eq!(
            url.as_str(),
            "http://plex.lan:32400/library/collections/5001/items/1001/move?after=999"
        );
    }

    #[test]
    fn the_removal_endpoint_addresses_one_child() {
        let url = client()
            .endpoint("library/collections/5001/items/1001", &[])
            .expect("a valid endpoint");
        assert_eq!(
            url.as_str(),
            "http://plex.lan:32400/library/collections/5001/items/1001"
        );
    }

    #[test]
    fn a_collection_children_answer_parses_as_a_page_of_items() {
        let body: ItemsBody = serde_json::from_str(
            r#"{"totalSize":2,"Metadata":[{"ratingKey":"1","type":"movie","title":"Alien"},
                {"ratingKey":"2","type":"movie","title":"Aliens"}]}"#,
        )
        .expect("parses");
        assert_eq!(body.metadata.len(), 2);
        assert_eq!(body.total_size, Some(2));
    }
}
