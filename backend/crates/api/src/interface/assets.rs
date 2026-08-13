// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The seam between the HTTP surface and the embedded bundle.

use std::{borrow::Cow, fmt};

/// One file out of the built SPA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    /// The bytes, borrowed from the binary's own image where possible.
    pub bytes: Cow<'static, [u8]>,
    /// The content type, decided by the embedder from the file's extension.
    pub content_type: String,
    /// Whether the path is content-addressed and may be cached forever.
    ///
    /// Vite fingerprints everything under `_app/immutable/`, so those may carry
    /// a year-long `Cache-Control`; `200.html` may not, or an upgraded binary
    /// serves a shell that loads bundles it no longer ships.
    pub immutable: bool,
}

/// Where the served SPA comes from.
///
/// A trait rather than a concrete type because the bundle is embedded by the
/// binary crate and this crate is one of its dependencies. It also lets the
/// route be tested without a build of the frontend.
pub trait AssetSource: fmt::Debug + Send + Sync + 'static {
    /// The asset at `path`, where `path` has no leading slash.
    fn get(&self, path: &str) -> Option<Asset>;

    /// The shell every unmatched route falls back to.
    ///
    /// `adapter-static` writes it as `200.html`. Returning `None` means no SPA
    /// was built into this binary, which the route reports as such rather than
    /// as a missing page.
    fn shell(&self) -> Option<Asset>;

    /// Every HTML document this build can serve, the shell included.
    ///
    /// The shell is not the only one. `adapter-static` prerenders each route to
    /// its own file — `index.html`, `dashboard.html`, and so on — and
    /// [`Self::get`] serves any of them by exact path, so a bookmark on
    /// `/index.html`, a crawler, or a proxy configured with `index index.html`
    /// is answered with a document the shell's own hash does not cover. The
    /// content policy is built from all of them, because a document whose
    /// inline bootstrap is not admitted renders as a blank page with the reason
    /// visible only in the browser console.
    fn documents(&self) -> Vec<Asset>;
}

/// An empty source, for a build with no SPA in it.
///
/// Exists so the API can be exercised — by tests, and by `cargo run` in a
/// checkout where the frontend has not been built — without pretending a shell
/// is there. Requesting a page then says the interface is absent, which is a
/// true statement, rather than answering 404 as though the route were wrong.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAssets;

impl AssetSource for NoAssets {
    fn get(&self, _path: &str) -> Option<Asset> {
        None
    }

    fn shell(&self) -> Option<Asset> {
        None
    }

    fn documents(&self) -> Vec<Asset> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_empty_source_reports_absence_rather_than_an_empty_page() {
        assert_eq!(NoAssets.get("favicon.svg"), None);
        assert_eq!(NoAssets.shell(), None);
        assert!(NoAssets.documents().is_empty());
    }
}
