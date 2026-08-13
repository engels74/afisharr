// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The router, and the calls that are about the server rather than its content.

use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::State,
    response::Response,
    routing::{delete, get, post, put},
};
use serde_json::{Value, json};

use crate::fake::{
    collection_routes, hub_routes, instance::FakeInstance, item_routes, json as shape,
    plan::FakeOperation,
};

/// The running fake, as an axum state.
pub(crate) type Running = Arc<FakeInstance>;

/// The routes, in the order this crate's modules call them.
pub(crate) fn router(running: Running) -> Router {
    Router::new()
        .route("/identity", get(identity))
        .route("/library/sections", get(sections))
        .route(
            "/library/sections/{key}/all",
            get(item_routes::items).put(item_routes::edit),
        )
        .route(
            "/library/sections/{key}/collections",
            get(collection_routes::collections),
        )
        // Static segments above take priority over this one, which is what
        // makes a discovered filter's own endpoint reachable without a second
        // router: the server composed `/library/sections/1/genre?type=1`, and
        // this is where it lands.
        .route(
            "/library/sections/{key}/{filter}",
            get(item_routes::choices),
        )
        .route("/library/metadata/{key}", get(item_routes::item))
        .route(
            "/library/metadata/{key}/posters",
            post(item_routes::upload_poster),
        )
        .route(
            "/library/collections",
            post(collection_routes::create_collection),
        )
        .route(
            "/library/collections/{key}",
            delete(collection_routes::delete_collection),
        )
        .route(
            "/library/collections/{key}/children",
            get(collection_routes::collection_items),
        )
        .route(
            "/library/collections/{key}/items",
            put(collection_routes::add_items),
        )
        .route(
            "/library/collections/{key}/items/{item}",
            delete(collection_routes::remove_item),
        )
        .route(
            "/library/collections/{key}/items/{item}/move",
            put(collection_routes::move_item),
        )
        .route("/hubs/sections/{key}/manage", get(hub_routes::hubs))
        .route(
            "/hubs/sections/{key}/manage/{hub}",
            put(hub_routes::set_visibility),
        )
        .route(
            "/hubs/sections/{key}/manage/{hub}/move",
            put(hub_routes::move_hub),
        )
        .with_state(running)
}

/// The query string, as the handlers read it.
pub(crate) type Params = HashMap<String, String>;

/// `GET /identity`.
async fn identity(State(running): State<Running>) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::Identity).await {
        return Err(refusal);
    }
    let world = running.world();
    Ok(Json(shape::container(&json!({
        "size": 0,
        "claimed": true,
        "machineIdentifier": world.machine_identifier,
        "version": world.version,
    }))))
}

/// `GET /library/sections`.
async fn sections(State(running): State<Running>) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::Sections).await {
        return Err(refusal);
    }
    let world = running.world();
    let directory: Vec<Value> = world.libraries.iter().map(shape::section).collect();
    Ok(Json(shape::container(&json!({
        "size": directory.len(),
        "Directory": directory,
    }))))
}

/// The window a listing call was asked for.
pub(crate) fn window(params: &Params) -> (usize, usize) {
    let start = params
        .get("X-Plex-Container-Start")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    // No window means the whole result, which is what a real server does. The
    // client never asks that way — `ItemQuery` has no unwindowed variant — and
    // the fake matching the server here is what keeps the contract test honest.
    let size = params
        .get("X-Plex-Container-Size")
        .and_then(|value| value.parse().ok())
        .unwrap_or(usize::MAX);
    (start, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(pairs: &[(&str, &str)]) -> Params {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn a_window_is_read_from_the_container_parameters() {
        assert_eq!(
            window(&query(&[
                ("X-Plex-Container-Start", "200"),
                ("X-Plex-Container-Size", "50"),
            ])),
            (200, 50)
        );
    }

    #[test]
    fn a_request_with_no_window_asks_for_everything() {
        assert_eq!(window(&query(&[])), (0, usize::MAX));
    }

    #[test]
    fn a_window_that_is_not_a_number_falls_back_rather_than_failing() {
        // A real server ignores what it cannot read here. The fake matching
        // that is what keeps a client's malformed request a client bug rather
        // than a fake-only failure.
        assert_eq!(
            window(&query(&[("X-Plex-Container-Start", "soon")])),
            (0, usize::MAX)
        );
    }
}
