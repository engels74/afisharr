// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `POST /library/metadata/{key}/posters` — putting a poster on an item.

use afisharr_sources::outbound::{HeaderName, HeaderValue, Method, RequestBody};

use crate::{
    libraries::RatingKey,
    server::{PlexServerClient, ServerError},
};

/// The header the image's own type is declared in.
const CONTENT_TYPE: HeaderName = HeaderName::from_static("content-type");

/// A poster ready to upload.
///
/// The bytes and their type travel together because Plex stores what it is
/// sent: an image labelled `image/jpeg` and encoded as PNG is a poster that
/// renders as a broken box in every client, and the two fields being one value
/// is what stops a call site pairing them wrongly.
#[derive(Clone, PartialEq, Eq)]
pub struct ArtworkUpload {
    bytes: Vec<u8>,
    content_type: HeaderValue,
}

impl ArtworkUpload {
    /// A PNG poster.
    #[must_use]
    pub fn png(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            content_type: HeaderValue::from_static("image/png"),
        }
    }

    /// A JPEG poster.
    #[must_use]
    pub fn jpeg(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            content_type: HeaderValue::from_static("image/jpeg"),
        }
    }

    /// How many bytes will go on the wire.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether there is nothing to upload.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl std::fmt::Debug for ArtworkUpload {
    /// Prints the size and the type, never a megabyte of image.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtworkUpload")
            .field("bytes", &self.bytes.len())
            .field("content_type", &self.content_type)
            .finish()
    }
}

impl PlexServerClient {
    /// Uploads a poster and makes it the item's selected one.
    ///
    /// Plex selects an uploaded poster on receipt, so this is one call and not
    /// an upload followed by a select. The original is captured before this
    /// runs, not by this call: `I-REV-2` requires the capture to be a separate,
    /// verified act, and a capture folded into the overwrite is a capture that
    /// cannot be audited (§16.1, P3).
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer or
    /// refused the upload.
    #[tracing::instrument(skip(self, poster))]
    pub async fn upload_poster(
        &self,
        rating_key: &RatingKey,
        poster: ArtworkUpload,
    ) -> Result<(), ServerError> {
        let url = self.endpoint(&format!("library/metadata/{rating_key}/posters"), &[])?;
        let content_type = poster.content_type.clone();
        self.send(
            Method::POST,
            &url,
            Some(RequestBody::Bytes(poster.bytes)),
            &[(CONTENT_TYPE, content_type)],
        )
        .await?;
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
    fn the_upload_endpoint_is_the_items_own_poster_collection() {
        let url = client()
            .endpoint("library/metadata/1001/posters", &[])
            .expect("a valid endpoint");
        assert_eq!(
            url.as_str(),
            "http://plex.lan:32400/library/metadata/1001/posters"
        );
    }

    #[test]
    fn a_poster_never_prints_its_own_bytes() {
        let poster = ArtworkUpload::png(vec![0x89, b'P', b'N', b'G', 0x00, 0x01]);
        let printed = format!("{poster:?}");
        assert!(printed.contains("bytes: 6"), "{printed}");
        assert!(!printed.contains("137"), "{printed}");
    }

    #[test]
    fn the_declared_type_travels_with_the_bytes() {
        assert_eq!(
            ArtworkUpload::jpeg(vec![0xff]).content_type,
            HeaderValue::from_static("image/jpeg")
        );
        assert_eq!(
            ArtworkUpload::png(vec![0x89]).content_type,
            HeaderValue::from_static("image/png")
        );
    }

    #[test]
    fn an_empty_poster_says_so_before_it_reaches_the_wire() {
        assert!(ArtworkUpload::png(Vec::new()).is_empty());
        assert_eq!(ArtworkUpload::png(vec![1, 2, 3]).len(), 3);
    }
}
