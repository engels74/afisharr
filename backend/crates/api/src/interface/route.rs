// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The fallback that serves the SPA.

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{
        HeaderValue, Request, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE},
    },
    response::{IntoResponse, Response},
};

use crate::{
    error::{AppError, ErrorCode},
    interface::Asset,
    state::ApiState,
};

/// A year, for the content-addressed bundles Vite fingerprints.
const IMMUTABLE: HeaderValue = HeaderValue::from_static("public, max-age=31536000, immutable");

/// No caching, for the shell that names which bundles to load.
const REVALIDATE: HeaderValue = HeaderValue::from_static("no-cache");

/// Serves an embedded asset, or the shell for any path the SPA routes itself.
///
/// Anything under `/api/` has already been matched or rejected by the router,
/// so a request arriving here is a page. Answering it with the shell is what
/// makes a deep link work on a full page load: the client router reads the URL
/// and renders the right route.
pub async fn spa(State(state): State<ApiState>, request: Request<Body>) -> Response {
    let path = asset_path(request.uri().path());
    let assets = state.assets();

    if let Some(asset) = assets.get(&path) {
        return serve(asset);
    }

    match assets.shell() {
        Some(shell) => serve(shell),
        // Not a 404: the route is right and the page is missing because this
        // binary was built without an interface. Saying so names the actual
        // problem instead of sending someone to look for a typo in the URL.
        None => AppError::of(
            ErrorCode::NotFound,
            "This build carries no interface. Build the SPA and rebuild the binary.",
        )
        .into_response(),
    }
}

/// The embedded bundle's key for the path a browser asked for.
///
/// `Uri::path()` returns the request target as it was sent, which is
/// percent-encoded, while [`AssetSource::get`] is an exact lookup against the
/// literal filenames `adapter-static` wrote. Any asset whose name needs
/// encoding therefore missed and fell through to the shell — so `static/logo
/// (1).png`, or any non-ASCII name an operator adds, was answered with the full
/// SPA document, `Content-Type: text/html`, and HTTP 200 in place of the image
/// bytes. A broken image with a 200 behind it and no 404 anywhere to explain it.
///
/// A target that is not valid UTF-8 once decoded is left as it arrived: it
/// names no file this build carries either way, and the shell is the honest
/// answer to a path the SPA may still route.
///
/// [`AssetSource::get`]: crate::interface::AssetSource::get
fn asset_path(target: &str) -> String {
    let raw = target.trim_start_matches('/');
    percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .map_or_else(|_| raw.to_owned(), std::borrow::Cow::into_owned)
}

/// Writes one asset out, without copying it.
///
/// By value, and matched rather than `into_owned`: [`Asset::bytes`] is borrowed
/// from the binary's own image for every embedded file, and `Bytes::from_static`
/// keeps it that way. Cloning the `Cow` and then owning it copied the whole file
/// per request — the ~300 KB entry bundle twice over, on a route that sits
/// outside every rate limit, so an unauthenticated caller could drive that churn
/// at line rate with no counter moving.
fn serve(asset: Asset) -> Response {
    let body = match asset.bytes {
        std::borrow::Cow::Borrowed(bytes) => Body::from(Bytes::from_static(bytes)),
        std::borrow::Cow::Owned(bytes) => Body::from(bytes),
    };
    let mut response = Response::new(body);
    *response.status_mut() = StatusCode::OK;
    if let Ok(content_type) = HeaderValue::from_str(&asset.content_type) {
        response.headers_mut().insert(CONTENT_TYPE, content_type);
    }
    response.headers_mut().insert(
        CACHE_CONTROL,
        if asset.immutable {
            IMMUTABLE
        } else {
            REVALIDATE
        },
    );
    response
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    fn asset(immutable: bool) -> Asset {
        Asset {
            bytes: Cow::Borrowed(b"body"),
            content_type: "text/html; charset=utf-8".to_owned(),
            immutable,
        }
    }

    #[test]
    fn a_fingerprinted_asset_is_cacheable_for_a_year() {
        let response = serve(asset(true));
        assert_eq!(response.headers().get(CACHE_CONTROL), Some(&IMMUTABLE));
    }

    #[test]
    fn the_shell_is_revalidated_on_every_load() {
        // An upgraded binary ships new bundle names; a cached shell would go on
        // asking for the ones it no longer carries.
        let response = serve(asset(false));
        assert_eq!(response.headers().get(CACHE_CONTROL), Some(&REVALIDATE));
    }

    #[test]
    fn an_encoded_name_resolves_to_the_file_the_bundle_carries() {
        // What the browser sends for `static/logo (1).png`. Looked up raw, this
        // missed and the operator was handed the SPA shell as the image.
        assert_eq!(asset_path("/logo%20(1).png"), "logo (1).png");
        assert_eq!(asset_path("/f%C3%A5rikk%C3%A5l.png"), "fårikkål.png");
    }

    #[test]
    fn an_unencoded_name_is_unchanged() {
        assert_eq!(
            asset_path("/_app/immutable/entry/app.js"),
            "_app/immutable/entry/app.js"
        );
        assert_eq!(asset_path("/dashboard"), "dashboard");
    }

    #[test]
    fn a_target_that_does_not_decode_is_left_as_it_arrived() {
        // `%ff` is not UTF-8. It names nothing either way; what matters is that
        // the route answers rather than failing on it.
        assert_eq!(asset_path("/%ff"), "%ff");
    }

    #[test]
    fn the_content_type_the_embedder_decided_is_the_one_sent() {
        let response = serve(asset(false));
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/html; charset=utf-8")
        );
    }
}
