// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What an outbound request carries, when it carries anything.

/// The body of one outbound request.
///
/// Two variants and not one `Vec<u8>`, because the two are different at the
/// call site and only one of them is readable in a log: a JSON document an
/// adapter composed, and an image an adapter is uploading. Collapsing them
/// would put a megabyte of PNG in reach of the first `?body` anybody writes.
#[derive(Clone, PartialEq, Eq)]
pub enum RequestBody {
    /// A text body — JSON, or a form-encoded document.
    Text(String),
    /// Raw bytes — an image being uploaded to Plex.
    Bytes(Vec<u8>),
}

impl RequestBody {
    /// How many bytes this body will put on the wire.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Bytes(bytes) => bytes.len(),
        }
    }

    /// Whether this body is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The body as the transport wants it.
    pub(super) fn into_bytes(self) -> Vec<u8> {
        match self {
            Self::Text(text) => text.into_bytes(),
            Self::Bytes(bytes) => bytes,
        }
    }
}

impl std::fmt::Debug for RequestBody {
    /// Prints the size, never the content.
    ///
    /// The derived implementation would print an uploaded poster byte by byte
    /// into whatever `?body` a `#[instrument]` attribute already captures, and
    /// a text body composed from a credential straight into the log D-032 says
    /// no secret reaches.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            Self::Text(_) => "text",
            Self::Bytes(_) => "bytes",
        };
        write!(f, "RequestBody::{kind}({} bytes)", self.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_reports_its_size_without_printing_itself() {
        let body = RequestBody::Text("{\"token\":\"secret\"}".to_owned());
        let printed = format!("{body:?}");
        assert!(!printed.contains("secret"), "{printed}");
        assert!(printed.contains("18 bytes"), "{printed}");
    }

    #[test]
    fn bytes_survive_the_trip_to_the_transport_unchanged() {
        let bytes = vec![0x89, b'P', b'N', b'G'];
        assert_eq!(RequestBody::Bytes(bytes.clone()).into_bytes(), bytes);
        assert_eq!(RequestBody::Text("ab".to_owned()).into_bytes(), b"ab");
    }

    #[test]
    fn an_empty_body_says_so() {
        assert!(RequestBody::Bytes(Vec::new()).is_empty());
        assert!(!RequestBody::Text("x".to_owned()).is_empty());
    }
}
