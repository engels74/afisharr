// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The fallback that serves the SPA.

use axum::{
    body::Body,
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
    let path = request.uri().path().trim_start_matches('/');
    let assets = state.assets();

    if let Some(asset) = assets.get(path) {
        return serve(&asset);
    }

    match assets.shell() {
        Some(shell) => serve(&shell),
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

fn serve(asset: &Asset) -> Response {
    let mut response = Response::new(Body::from(asset.bytes.clone().into_owned()));
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
        let response = serve(&asset(true));
        assert_eq!(response.headers().get(CACHE_CONTROL), Some(&IMMUTABLE));
    }

    #[test]
    fn the_shell_is_revalidated_on_every_load() {
        // An upgraded binary ships new bundle names; a cached shell would go on
        // asking for the ones it no longer carries.
        let response = serve(&asset(false));
        assert_eq!(response.headers().get(CACHE_CONTROL), Some(&REVALIDATE));
    }

    #[test]
    fn the_content_type_the_embedder_decided_is_the_one_sent() {
        let response = serve(&asset(false));
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/html; charset=utf-8")
        );
    }
}
