// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The one edit endpoint Plex has, and everything that goes through it.
//!
//! `PUT /library/sections/{key}/all` writes whatever `id` names, at the libtype
//! `type` names (`plexapi/library.py:1743-1755`). A collection, an item, and a
//! tag edit are the same request with different arguments — so the request is
//! built once here and the three callers contribute only their own arguments
//! (P7).
//!
//! **The answer is a count, and the count is a fact.** An edit naming a key the
//! server does not hold writes nothing and says so. A caller that discarded the
//! number could not tell that from a success, which is how a pass comes to
//! believe it renamed a collection somebody else had deleted.

use afisharr_sources::outbound::Method;
use serde::Deserialize;

use crate::{
    libraries::{ItemKind, RatingKey, SectionKey},
    server::{PlexServerClient, ServerError},
};

/// What an edit answers: how many rows it wrote.
#[derive(Debug, Deserialize)]
struct WrittenBody {
    #[serde(default)]
    size: Option<u32>,
}

impl PlexServerClient {
    /// Sends one edit and reads how many rows it wrote.
    ///
    /// The ids travel as one comma-joined argument, which is what a real client
    /// sends (`plexapi/library.py:1749`).
    pub(crate) async fn edit_at(
        &self,
        section: &SectionKey,
        libtype: ItemKind,
        ids: &[RatingKey],
        arguments: Vec<(String, String)>,
    ) -> Result<usize, ServerError> {
        if ids.is_empty() {
            return Err(ServerError::Incomplete {
                call: "PUT /library/sections/{id}/all",
                missing: "any item to edit",
            });
        }
        let joined = ids
            .iter()
            .map(RatingKey::to_string)
            .collect::<Vec<String>>()
            .join(",");
        let mut query = vec![
            ("type".to_owned(), libtype.as_plex_type().to_string()),
            ("id".to_owned(), joined),
        ];
        query.extend(arguments);
        let url = self.endpoint(&format!("library/sections/{section}/all"), &query)?;
        let response = self.send(Method::PUT, &url, None, &[]).await?;
        // An answer with no body at all is the server declining to say, not the
        // transport failing: a reference client tolerates exactly this shape on
        // a write — it accepts `204` and reads a blank body as `None` rather
        // than as a parse error (`plexapi/server.py:759`,
        // `plexapi/utils.py:836-839`). Reported as [`ServerError::Incomplete`]
        // so an operator reading it is told the server said nothing about the
        // write, rather than that the server could not be reached.
        let body: WrittenBody = if response.body.trim().is_empty() {
            WrittenBody { size: None }
        } else {
            self.parse_container(&response)?
        };
        // A server that did not report a size did not say what it wrote, and
        // "did not say" is not "wrote nothing" (P1). Refused rather than
        // defaulted, because the caller's next decision turns on the number.
        body.size
            .map(|size| size as usize)
            .ok_or(ServerError::Incomplete {
                call: "PUT /library/sections/{id}/all",
                missing: "how many rows it wrote",
            })
    }

    /// Writes one item's sort title: its value, its presence, and its lock.
    ///
    /// The write half of the round trip §15.6 requires, and the call this
    /// build did not have: every edit it could send named the collection
    /// libtype, so an item's sort title was unreachable and the capture had
    /// nothing to restore through.
    ///
    /// `None` clears the attribute. Plex has no other way to say it: the value
    /// goes out empty, and the server answers the item with no `titleSort` at
    /// all afterwards — which is the state a teardown has to be able to reach,
    /// because it is the state most items start in (`I-REV-3`).
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer, and
    /// [`ServerError::Incomplete`] when it answered without saying what it
    /// wrote.
    #[tracing::instrument(skip(self))]
    pub async fn edit_item_sort_title(
        &self,
        section: &SectionKey,
        libtype: ItemKind,
        item: &RatingKey,
        value: Option<&str>,
        locked: bool,
    ) -> Result<usize, ServerError> {
        let arguments = vec![
            (
                "titleSort.value".to_owned(),
                value.unwrap_or_default().to_owned(),
            ),
            ("titleSort.locked".to_owned(), i32::from(locked).to_string()),
        ];
        self.edit_at(section, libtype, std::slice::from_ref(item), arguments)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct Envelope {
        #[serde(rename = "MediaContainer")]
        media_container: WrittenBody,
    }

    fn written(json: &str) -> Option<u32> {
        serde_json::from_str::<Envelope>(json)
            .expect("parses")
            .media_container
            .size
    }

    #[test]
    fn an_edit_that_wrote_nothing_says_zero_rather_than_saying_nothing() {
        assert_eq!(written(r#"{"MediaContainer":{"size":0}}"#), Some(0));
        assert_eq!(written(r#"{"MediaContainer":{"size":3}}"#), Some(3));
    }

    #[test]
    fn a_server_that_did_not_report_a_size_is_absent_rather_than_zero() {
        // "Did not say" and "wrote nothing" send a caller in opposite
        // directions, and only one of them means the target is gone (P1).
        assert_eq!(written(r#"{"MediaContainer":{}}"#), None);
    }
}
