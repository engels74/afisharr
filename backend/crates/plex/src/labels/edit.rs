// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Adding and removing labels on one item.

use afisharr_sources::outbound::Method;

use crate::{
    libraries::{ItemKind, RatingKey, SectionKey},
    server::{PlexServerClient, ServerError},
};

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
    /// Additions are indexed because Plex reads them as an array; removals use
    /// the empty-index subtraction form, which is a set difference rather than
    /// a positional write. The lock is written to `0` explicitly: Plex locks a
    /// tag field it is told to write unless told otherwise, and a locked label
    /// field stops the operator editing labels in Plex ever again.
    #[must_use]
    pub fn pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::with_capacity(self.add.len() + self.remove.len() + 1);
        for (index, label) in self.add.iter().enumerate() {
            pairs.push((format!("label[{index}].tag.tag"), label.clone()));
        }
        for label in &self.remove {
            pairs.push(("label[].tag.tag-".to_owned(), label.clone()));
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
    ) -> Result<(), ServerError> {
        if edit.is_empty() {
            return Err(ServerError::Incomplete {
                call: "PUT /library/sections/{id}/all",
                missing: "any label to add or remove",
            });
        }
        let mut query = vec![
            ("type".to_owned(), libtype.as_plex_type().to_string()),
            ("id".to_owned(), item.to_string()),
        ];
        query.extend(edit.pairs());
        let url = self.endpoint(&format!("library/sections/{section}/all"), &query)?;
        self.send(Method::PUT, &url, None, &[]).await?;
        Ok(())
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
