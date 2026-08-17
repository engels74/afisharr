// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Adding and removing labels on one item.

use percent_encoding::{AsciiSet, CONTROLS};

use crate::{
    libraries::{ItemKind, RatingKey, SectionKey},
    server::{PlexServerClient, ServerError},
};

/// What a tag value is quoted against inside the comma-joined removal list.
///
/// Python's `quote` leaves `/` and the unreserved set alone and escapes
/// everything else, and the comma is the byte that matters: it is the
/// separator, so a label containing one has to arrive escaped or the server
/// reads it as two labels and removes neither.
const QUOTED: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b',')
    .add(b'&')
    .add(b'=')
    .add(b'?')
    .add(b'#')
    .add(b'%')
    .add(b'+');

/// Labels to add to an item, and labels to take off it.
///
/// Both in one value because Plex applies both in one request, and two requests
/// would leave a window in which an item carries neither the old label nor the
/// new one — which a pass running alongside reads as an unmanaged item.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LabelEdit {
    /// Labels to put on the item.
    pub add: Vec<String>,
    /// Labels to take off it.
    pub remove: Vec<String>,
}

impl LabelEdit {
    /// An edit that adds one label.
    #[must_use]
    pub fn adding(label: impl Into<String>) -> Self {
        Self {
            add: vec![label.into()],
            remove: Vec::new(),
        }
    }

    /// An edit that removes one label.
    #[must_use]
    pub fn removing(label: impl Into<String>) -> Self {
        Self {
            add: Vec::new(),
            remove: vec![label.into()],
        }
    }

    /// Whether this edit would write anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }

    /// The query pairs this edit contributes.
    ///
    /// Additions are indexed because Plex reads them as an array. Removals are
    /// one argument, not one per label: `label[].tag.tag-` carries every
    /// removed tag joined with commas, each percent-quoted on its own so a
    /// label holding a comma survives the join
    /// (`plexapi/mixins/edit.py:331-333`). Sent as a repeated key, a two-label
    /// removal removed one label on a real server and reported success.
    ///
    /// The lock is written to `0` explicitly: Plex locks a tag field it is told
    /// to write unless told otherwise, and a locked label field stops the
    /// operator editing labels in Plex ever again.
    #[must_use]
    pub fn pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::with_capacity(self.add.len() + 2);
        for (index, label) in self.add.iter().enumerate() {
            pairs.push((format!("label[{index}].tag.tag"), label.clone()));
        }
        if !self.remove.is_empty() {
            let joined = self
                .remove
                .iter()
                .map(|label| percent_encoding::utf8_percent_encode(label, QUOTED).to_string())
                .collect::<Vec<String>>()
                .join(",");
            pairs.push(("label[].tag.tag-".to_owned(), joined));
        }
        if !pairs.is_empty() {
            pairs.push(("label.locked".to_owned(), "0".to_owned()));
        }
        pairs
    }
}

impl PlexServerClient {
    /// Applies a label edit to one item.
    ///
    /// Answers how many rows the server says it wrote, for the reason every
    /// edit does: an item re-keyed under the caller is an edit that wrote
    /// nothing, and only the count says so.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer, and
    /// [`ServerError::Incomplete`] when the edit names no label — a request
    /// with no tag arguments is a `PUT` with no effect reported as a success.
    #[tracing::instrument(skip(self))]
    pub async fn edit_labels(
        &self,
        section: &SectionKey,
        libtype: ItemKind,
        item: &RatingKey,
        edit: &LabelEdit,
    ) -> Result<usize, ServerError> {
        if edit.is_empty() {
            return Err(ServerError::Incomplete {
                call: "PUT /library/sections/{id}/all",
                missing: "any label to add or remove",
            });
        }
        self.edit_at(section, libtype, std::slice::from_ref(item), edit.pairs())
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_addition_is_written_as_an_indexed_array_entry() {
        assert_eq!(
            LabelEdit::adding("afisharr").pairs(),
            vec![
                ("label[0].tag.tag".to_owned(), "afisharr".to_owned()),
                ("label.locked".to_owned(), "0".to_owned()),
            ]
        );
    }

    #[test]
    fn a_removal_uses_the_subtraction_form_rather_than_a_positional_write() {
        // A positional write would replace the whole tag list, dropping every
        // label the operator added in Plex.
        assert_eq!(
            LabelEdit::removing("afisharr").pairs(),
            vec![
                ("label[].tag.tag-".to_owned(), "afisharr".to_owned()),
                ("label.locked".to_owned(), "0".to_owned()),
            ]
        );
    }

    #[test]
    fn two_removals_travel_as_one_comma_joined_argument() {
        // One argument, not one per label. Sent as a repeated key this removed
        // the last label and answered success for both.
        let edit = LabelEdit {
            add: Vec::new(),
            remove: vec!["old".to_owned(), "older".to_owned()],
        };
        assert_eq!(
            edit.pairs()[0],
            ("label[].tag.tag-".to_owned(), "old,older".to_owned())
        );
        assert_eq!(edit.pairs().len(), 2, "the removal and the lock");
    }

    #[test]
    fn a_label_holding_a_comma_is_quoted_so_the_join_stays_unambiguous() {
        // The comma is the separator. Unescaped, a label called "a,b" reads as
        // two labels and neither of them exists.
        let edit = LabelEdit {
            add: Vec::new(),
            remove: vec!["a,b".to_owned(), "c".to_owned()],
        };
        assert_eq!(
            edit.pairs()[0],
            ("label[].tag.tag-".to_owned(), "a%2Cb,c".to_owned())
        );
    }

    #[test]
    fn additions_and_removals_travel_in_one_request() {
        let edit = LabelEdit {
            add: vec!["new".to_owned()],
            remove: vec!["old".to_owned()],
        };
        let pairs = edit.pairs();
        assert!(pairs.contains(&("label[0].tag.tag".to_owned(), "new".to_owned())));
        assert!(pairs.contains(&("label[].tag.tag-".to_owned(), "old".to_owned())));
    }

    #[test]
    fn several_additions_are_indexed_in_order() {
        let edit = LabelEdit {
            add: vec!["a".to_owned(), "b".to_owned()],
            remove: Vec::new(),
        };
        let pairs = edit.pairs();
        assert_eq!(pairs[0].0, "label[0].tag.tag");
        assert_eq!(pairs[1].0, "label[1].tag.tag");
    }

    #[test]
    fn the_label_field_is_left_unlocked() {
        // Plex locks a tag field it writes unless told otherwise, and a locked
        // label field stops the operator editing labels in Plex ever again.
        assert!(
            LabelEdit::adding("x")
                .pairs()
                .contains(&("label.locked".to_owned(), "0".to_owned()))
        );
    }

    #[test]
    fn an_edit_that_names_no_label_writes_nothing_at_all() {
        assert!(LabelEdit::default().is_empty());
        assert!(LabelEdit::default().pairs().is_empty());
    }
}
