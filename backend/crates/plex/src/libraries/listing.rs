// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `GET /library/sections/{key}/all` — one window of a library.

use afisharr_sources::outbound::Method;
use serde::Deserialize;

use crate::{
    libraries::{ItemQuery, LibraryItem, SectionKey, Window, item::ItemBody},
    server::{PlexServerClient, ServerError},
};

/// One window of a library, and how much of the library it was.
///
/// `total` is what the server said the whole result is, and `None` means it did
/// not say. A caller that read a missing total as zero would stop paging at the
/// first window and call the rest of the library absent (P1).
#[derive(Debug, Clone, PartialEq)]
pub struct ItemPage {
    /// The items in this window.
    pub items: Vec<LibraryItem>,
    /// The size of the whole result, when the server reported one.
    pub total: Option<u32>,
    /// The window that was asked for.
    pub window: Window,
}

impl ItemPage {
    /// Whether another window is worth asking for.
    ///
    /// Two conditions, and both are needed. A short page is the end of the
    /// result whatever the total says; and a full page with no reported total
    /// is still worth continuing from, because "the server did not say" is not
    /// "there is no more".
    #[must_use]
    pub fn has_more(&self) -> bool {
        // A window of nothing is never worth advancing, whatever the total says:
        // it can never be short of what was asked for, and `Window::next` on it
        // returns the same window — so a caller paging on this would re-request
        // the empty window for ever. The zero-size window is not hypothetical:
        // it is how the filter vocabulary is asked for.
        if self.window.size == 0 {
            return false;
        }
        let filled = u32::try_from(self.items.len()).unwrap_or(u32::MAX) >= self.window.size;
        if !filled {
            // Short of what was asked for is the end of the result, whatever the
            // total says. A server whose reported total outruns what it actually
            // returned — items removed between windows, or a total counted before
            // a filter — would otherwise have the caller request window after
            // empty window until the arithmetic caught up.
            return false;
        }
        match self.total {
            Some(total) => self.window.start.saturating_add(self.window.size) < total,
            None => true,
        }
    }
}

/// The item list `GET /library/sections/{key}/all` answers with.
#[derive(Debug, Deserialize)]
pub(crate) struct ItemsBody {
    #[serde(default, rename = "Metadata")]
    pub(crate) metadata: Vec<ItemBody>,
    #[serde(default, rename = "totalSize")]
    pub(crate) total_size: Option<u32>,
}

impl PlexServerClient {
    /// Reads one window of a library.
    ///
    /// Windowed and never whole: `I-PERF-1` bounds the working set by batch
    /// size rather than by library size, and the way to keep that true is for
    /// the only listing call in the crate to take a [`Window`].
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer or
    /// refused.
    #[tracing::instrument(skip(self, query))]
    pub async fn items(
        &self,
        section: &SectionKey,
        query: &ItemQuery,
    ) -> Result<ItemPage, ServerError> {
        let url = self.endpoint(&format!("library/sections/{section}/all"), &query.pairs())?;
        let body: ItemsBody = self.container(Method::GET, &url, None).await?;
        Ok(ItemPage {
            items: body.metadata.into_iter().map(LibraryItem::from).collect(),
            total: body.total_size,
            window: query.window(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(json: &str, window: Window) -> ItemPage {
        let body: ItemsBody = serde_json::from_str(json).expect("parses");
        ItemPage {
            items: body.metadata.into_iter().map(LibraryItem::from).collect(),
            total: body.total_size,
            window,
        }
    }

    const TWO_ITEMS: &str = r#"{
      "size": 2,
      "totalSize": 1200,
      "Metadata": [
        {"ratingKey":"1","type":"movie","title":"Alien","year":1979},
        {"ratingKey":"2","type":"movie","title":"Aliens","year":1986}
      ]
    }"#;

    #[test]
    fn a_window_reads_its_items_and_the_whole_result_size() {
        let page = page(TWO_ITEMS, Window { start: 0, size: 2 });
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.total, Some(1200));
        assert_eq!(page.items[1].title, "Aliens");
    }

    #[test]
    fn a_window_short_of_the_total_asks_for_another() {
        assert!(page(TWO_ITEMS, Window { start: 0, size: 2 }).has_more());
    }

    #[test]
    fn the_last_window_stops() {
        let page = page(
            TWO_ITEMS,
            Window {
                start: 1198,
                size: 2,
            },
        );
        assert!(!page.has_more());
    }

    #[test]
    fn a_short_window_stops_even_when_the_total_says_otherwise() {
        // Items removed between windows, or a total counted before a filter
        // narrowed the result: the server still claims 1200 and returned two of
        // a window of fifty. Reading the total alone would send the caller
        // through twenty-four more windows with nothing in any of them.
        let page = page(TWO_ITEMS, Window { start: 0, size: 50 });
        assert_eq!(page.total, Some(1200));
        assert!(!page.has_more());
    }

    #[test]
    fn a_full_window_with_no_reported_total_keeps_going() {
        // "The server did not say how many there are" is not "there are none
        // left". Stopping here would call the rest of a library absent.
        let page = page(
            r#"{"Metadata":[{"ratingKey":"1","type":"movie"},{"ratingKey":"2","type":"movie"}]}"#,
            Window { start: 0, size: 2 },
        );
        assert_eq!(page.total, None);
        assert!(page.has_more());
    }

    #[test]
    fn a_short_window_with_no_reported_total_stops() {
        let page = page(
            r#"{"Metadata":[{"ratingKey":"1","type":"movie"}]}"#,
            Window {
                start: 0,
                size: 200,
            },
        );
        assert!(!page.has_more());
    }

    #[test]
    fn a_window_of_nothing_is_never_worth_advancing() {
        // How the filter vocabulary is asked for. `Window::next` on it returns
        // the same window, so a caller paging on `has_more` would re-request
        // `start=0, size=0` for ever against a server reporting any total.
        let page = page(r#"{"totalSize":1200}"#, Window::first(0));
        assert_eq!(page.total, Some(1200));
        assert!(!page.has_more());
    }

    #[test]
    fn an_empty_answer_is_an_empty_page_and_not_a_failure() {
        // Whether an empty library is a fact or a failed fetch is the caller's
        // question, and it is answered by whether this returned `Ok` at all.
        let page = page(r#"{"size":0,"totalSize":0}"#, Window::first(200));
        assert!(page.items.is_empty());
        assert_eq!(page.total, Some(0));
        assert!(!page.has_more());
    }
}
