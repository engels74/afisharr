// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `GET /library/metadata/{key}` — one item's media facts.

use afisharr_sources::outbound::Method;

use crate::{
    libraries::listing::ItemsBody,
    libraries::{LibraryItem, RatingKey},
    server::{PlexServerClient, ServerError},
};

impl PlexServerClient {
    /// Reads one item, with its media, parts, and streams.
    ///
    /// A separate call from the library listing because the two are asked at
    /// different rates: a pass lists a library in windows and reads one item's
    /// streams only when the overlay it renders depends on them.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer or
    /// refused, and [`ServerError::Incomplete`] when it answered with no item —
    /// which is not the same as an item with nothing in it.
    #[tracing::instrument(skip(self))]
    pub async fn item(&self, rating_key: &RatingKey) -> Result<LibraryItem, ServerError> {
        let url = self.endpoint(&format!("library/metadata/{rating_key}"), &[])?;
        let body: ItemsBody = self.container(Method::GET, &url, None).await?;
        body.metadata
            .into_iter()
            .next()
            .map(LibraryItem::from)
            .ok_or(ServerError::Incomplete {
                call: "GET /library/metadata/{ratingKey}",
                missing: "the item it was asked for",
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        libraries::{LibraryItem, listing::ItemsBody},
        streams::StreamKind,
    };

    fn only_item(json: &str) -> Option<LibraryItem> {
        let body: ItemsBody = serde_json::from_str(json).expect("parses");
        body.metadata.into_iter().next().map(LibraryItem::from)
    }

    #[test]
    fn an_item_answer_carries_its_streams_through_to_the_domain_type() {
        let item = only_item(
            r#"{"Metadata":[{"ratingKey":"1001","type":"movie","title":"Alien",
                "Media":[{"container":"mkv","videoResolution":"1080",
                  "Part":[{"file":"/data/Alien.mkv","exists":true,
                    "Stream":[{"streamType":2,"codec":"dts","channels":6}]}]}]}]}"#,
        )
        .expect("one item");
        let media = item.media().expect("the scan is complete");
        assert_eq!(media[0].video_resolution.as_deref(), Some("1080"));
        let stream = &media[0].parts[0].streams[0];
        assert_eq!(stream.kind, StreamKind::Audio);
        assert_eq!(stream.channels, Some(6));
    }

    #[test]
    fn an_answer_with_no_item_is_told_apart_from_an_item_with_no_media() {
        // The rating key was rebound or the item was deleted, and neither is
        // "this film has no audio track" (P1).
        assert!(only_item(r#"{"size":0}"#).is_none());
        let empty =
            only_item(r#"{"Metadata":[{"ratingKey":"1","type":"movie"}]}"#).expect("one item");
        assert_eq!(empty.media(), Some(&[][..]));
    }
}
