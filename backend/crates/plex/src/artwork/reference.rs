// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Classifying a poster reference, including the ones this build cannot read.

/// What shape a poster reference is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkKind {
    /// A server-relative path, e.g. `/library/metadata/1/thumb/17000`.
    ServerPath,
    /// An absolute `http`/`https` URL, as an agent supplies for a remote asset.
    AbsoluteUrl,
    /// A scheme Plex uses internally, e.g. `upload://` or `media://`.
    ///
    /// Distinct from [`Self::Unrecognised`]: it is a shape this build knows the
    /// name of and cannot fetch, which is a different report to the operator
    /// than a shape nobody has seen before.
    InternalScheme,
    /// Something this build does not recognise at all.
    Unrecognised,
}

/// A reference to a poster, as the server reported it.
///
/// The raw text is always kept. `I-RENDER-2` needs the base poster restored
/// byte-exactly from where it came from, and a reference normalised into a
/// shape this build prefers is a reference that no longer points where Plex
/// said it did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkRef {
    raw: String,
    kind: ArtworkKind,
}

impl ArtworkRef {
    /// Classifies a reference without failing on one it cannot read.
    ///
    /// Never returns an error, and that is the rule rather than laziness: a
    /// library pass that aborted on an artwork URL format would stop the whole
    /// sync over a field no collection depends on (`I-ID-2`).
    #[must_use]
    pub fn classify(raw: &str) -> Self {
        let trimmed = raw.trim();
        let kind = if trimmed.is_empty() {
            ArtworkKind::Unrecognised
        } else if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            ArtworkKind::AbsoluteUrl
        } else if trimmed.starts_with('/') {
            ArtworkKind::ServerPath
        } else if internal_scheme(trimmed) {
            ArtworkKind::InternalScheme
        } else {
            ArtworkKind::Unrecognised
        };
        Self {
            raw: raw.to_owned(),
            kind,
        }
    }

    /// The reference exactly as the server sent it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// What shape it is in.
    #[must_use]
    pub const fn kind(&self) -> ArtworkKind {
        self.kind
    }

    /// Whether this build can turn the reference into a request.
    ///
    /// The one question a fetch asks. Everything else about the reference is
    /// for the record the doctor page reads.
    #[must_use]
    pub const fn is_fetchable(&self) -> bool {
        matches!(
            self.kind,
            ArtworkKind::ServerPath | ArtworkKind::AbsoluteUrl
        )
    }
}

/// Whether the reference names a scheme Plex uses internally.
fn internal_scheme(reference: &str) -> bool {
    const KNOWN: [&str; 3] = ["upload://", "media://", "metadata://"];
    KNOWN.iter().any(|scheme| reference.starts_with(scheme))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_relative_path_is_fetchable() {
        let reference = ArtworkRef::classify("/library/metadata/1001/thumb/1700000000");
        assert_eq!(reference.kind(), ArtworkKind::ServerPath);
        assert!(reference.is_fetchable());
    }

    #[test]
    fn an_absolute_url_is_fetchable() {
        let reference = ArtworkRef::classify("https://image.tmdb.org/t/p/original/abc.jpg");
        assert_eq!(reference.kind(), ArtworkKind::AbsoluteUrl);
        assert!(reference.is_fetchable());
    }

    #[test]
    fn an_internal_scheme_is_named_rather_than_called_unknown() {
        for reference in ["upload://posters/abc123", "media://42/thumb"] {
            let classified = ArtworkRef::classify(reference);
            assert_eq!(
                classified.kind(),
                ArtworkKind::InternalScheme,
                "{reference}"
            );
            assert!(!classified.is_fetchable());
        }
    }

    #[test]
    fn a_format_this_build_has_never_seen_is_recorded_and_not_fatal() {
        // `I-ID-2`: the pass continues, and the raw value survives so the
        // doctor page can report what was actually seen.
        let reference = ArtworkRef::classify("blorp:?id=17");
        assert_eq!(reference.kind(), ArtworkKind::Unrecognised);
        assert!(!reference.is_fetchable());
        assert_eq!(reference.as_str(), "blorp:?id=17");
    }

    #[test]
    fn an_empty_reference_is_unrecognised_rather_than_a_server_path() {
        assert_eq!(ArtworkRef::classify("").kind(), ArtworkKind::Unrecognised);
    }

    #[test]
    fn the_raw_text_is_never_normalised() {
        // A reference rewritten into a preferred shape no longer points where
        // Plex said it did, and `I-RENDER-2` restores from where it came from.
        let raw = "  /library/metadata/1/thumb/17  ";
        assert_eq!(ArtworkRef::classify(raw).as_str(), raw);
        assert_eq!(ArtworkRef::classify(raw).kind(), ArtworkKind::ServerPath);
    }
}
