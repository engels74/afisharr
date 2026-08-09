// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `rust-embed` over `frontend/build`.

use std::borrow::Cow;

use afisharr_api::interface::{Asset, AssetSource};
use rust_embed::Embed;

/// The shell `adapter-static` writes, which every unmatched route falls back to.
const SHELL: &str = "200.html";

/// Where Vite puts the content-addressed bundles.
///
/// Everything under it carries a hash in its filename, so it can be cached for
/// a year. Everything outside it cannot.
const FINGERPRINTED: &str = "_app/immutable/";

/// The built SPA, as files.
///
/// `debug-embed` is not enabled: a debug build reads `frontend/build` from disk
/// at runtime, so `bun run dev`-style iteration does not need a Rust rebuild,
/// while the release binary carries the bytes.
#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/../../../frontend/build"]
struct BuiltSpa;

/// The interface this binary serves.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmbeddedInterface;

impl EmbeddedInterface {
    /// Whether a shell was built into this binary at all.
    #[must_use]
    pub fn is_present() -> bool {
        BuiltSpa::get(SHELL).is_some()
    }
}

impl AssetSource for EmbeddedInterface {
    fn get(&self, path: &str) -> Option<Asset> {
        // A directory request has no file of its own; the shell answers it, and
        // the fallback in the route is what does that.
        if path.is_empty() || path.ends_with('/') {
            return None;
        }
        let file = BuiltSpa::get(path)?;
        Some(Asset {
            bytes: Cow::Owned(file.data.into_owned()),
            content_type: content_type_of(path),
            immutable: path.starts_with(FINGERPRINTED),
        })
    }

    fn shell(&self) -> Option<Asset> {
        let file = BuiltSpa::get(SHELL)?;
        Some(Asset {
            bytes: Cow::Owned(file.data.into_owned()),
            content_type: "text/html; charset=utf-8".to_owned(),
            // Never: an upgraded binary ships new bundle names, and a cached
            // shell would go on asking for the ones it no longer carries.
            immutable: false,
        })
    }
}

/// The content type for a path, from its extension.
///
/// `mime_guess` decides, and text types get a charset. Without one a browser
/// sniffs, and `X-Content-Type-Options: nosniff` then refuses the stylesheet.
fn content_type_of(path: &str) -> String {
    let guess = mime_guess::from_path(path).first_or_octet_stream();
    if guess.type_() == mime_guess::mime::TEXT
        || guess.essence_str() == "application/javascript"
        || guess.essence_str() == "image/svg+xml"
    {
        format!("{guess}; charset=utf-8")
    } else {
        guess.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_types_carry_a_charset_so_nosniff_does_not_refuse_them() {
        assert_eq!(content_type_of("app.css"), "text/css; charset=utf-8");
        assert_eq!(content_type_of("200.html"), "text/html; charset=utf-8");
        assert_eq!(
            content_type_of("favicon.svg"),
            "image/svg+xml; charset=utf-8"
        );
    }

    #[test]
    fn a_binary_type_carries_no_charset() {
        assert_eq!(content_type_of("poster.png"), "image/png");
    }

    #[test]
    fn an_unknown_extension_is_served_as_bytes_rather_than_guessed_at() {
        assert_eq!(
            content_type_of("thing.unknownext"),
            "application/octet-stream"
        );
    }

    #[test]
    fn only_the_fingerprinted_bundles_are_cacheable_forever() {
        assert!("_app/immutable/chunks/a.js".starts_with(FINGERPRINTED));
        assert!(!"favicon.svg".starts_with(FINGERPRINTED));
    }

    #[test]
    fn a_directory_request_falls_through_to_the_shell() {
        assert!(EmbeddedInterface.get("").is_none());
        assert!(EmbeddedInterface.get("settings/").is_none());
    }
}
