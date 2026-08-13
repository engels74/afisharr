// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The envelope every Plex answer arrives in.

use serde::Deserialize;

/// A `MediaContainer`, whatever it contains.
///
/// Plex wraps every JSON answer in one, so unwrapping it is done here once
/// rather than by a `#[serde(rename = "MediaContainer")]` field on a dozen
/// bodies (P7). Unknown fields are accepted deliberately: Plex adds them
/// between point releases, and a client that refused a body over one would go
/// down on an upgrade nobody here controls.
#[derive(Debug, Deserialize)]
pub(crate) struct Container<T> {
    #[serde(rename = "MediaContainer")]
    pub(crate) media_container: T,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct Inner {
        size: u32,
    }

    #[test]
    fn the_envelope_is_unwrapped_once() {
        let container: Container<Inner> =
            serde_json::from_str(r#"{"MediaContainer":{"size":3}}"#).expect("parses");
        assert_eq!(container.media_container.size, 3);
    }

    #[test]
    fn a_field_a_later_plex_adds_does_not_break_the_parse() {
        // Plex adds fields between point releases, and a client that refused a
        // body over one would go down on an upgrade nobody here controls.
        let container: Container<Inner> =
            serde_json::from_str(r#"{"MediaContainer":{"size":2,"newThing":true}}"#)
                .expect("parses");
        assert_eq!(container.media_container.size, 2);
    }

    #[test]
    fn an_answer_without_the_envelope_does_not_parse() {
        // A bare body is an answer from something that is not Plex — a captive
        // portal, or a proxy's own error page — and reading it as a container
        // would hand the caller an empty result as a fact (P1).
        assert!(serde_json::from_str::<Container<Inner>>(r#"{"size":3}"#).is_err());
    }
}
