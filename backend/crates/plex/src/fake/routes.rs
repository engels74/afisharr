// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The router, and the calls that are about the server rather than its content.

use std::sync::Arc;

use axum::{
    Router,
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::{delete, get, post, put},
};

use crate::fake::{
    collection_routes,
    element::Element,
    hub_routes,
    instance::FakeInstance,
    item_routes,
    negotiation::{Answer, Rendering},
    plan::FakeOperation,
    request::{self, Arguments},
    shape,
};

/// The running fake, as an axum state.
pub(crate) type Running = Arc<FakeInstance>;

/// The routes, in the order this crate's modules call them.
///
/// Two families answer a collection's items — `/library/metadata/{key}/…` and
/// `/library/collections/{key}/…` — and both are served. `python-plexapi` uses
/// only the first (`plexapi/collection.py:198`, `:332`, `:353`, `:372`) and
/// this crate's client uses only the second, and which family a real server
/// serves is settled by the contract test rather than here. Serving both is
/// what stops the fake asserting an answer either way.
pub(crate) fn router(running: Running) -> Router {
    Router::new()
        .route("/", get(root))
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
        .route(
            "/library/metadata/{key}",
            get(item_routes::item).delete(collection_routes::delete_collection),
        )
        .route(
            "/library/metadata/{key}/posters",
            post(item_routes::upload_poster),
        )
        .route(
            "/library/metadata/{key}/children",
            get(collection_routes::collection_items),
        )
        .route(
            "/library/metadata/{key}/items",
            put(collection_routes::add_items),
        )
        .route(
            "/library/metadata/{key}/items/{item}",
            delete(collection_routes::remove_item),
        )
        .route(
            "/library/metadata/{key}/items/{item}/move",
            put(collection_routes::move_item),
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
        .route(
            "/hubs/sections/{key}/manage",
            get(hub_routes::hubs)
                .post(hub_routes::promote)
                .delete(hub_routes::remove_every_hub),
        )
        .route(
            "/hubs/sections/{key}/manage/{hub}",
            put(hub_routes::set_visibility).delete(hub_routes::remove_hub),
        )
        .route(
            "/hubs/sections/{key}/manage/{hub}/move",
            put(hub_routes::move_hub),
        )
        .layer(middleware::from_fn_with_state(
            Arc::clone(&running),
            require_token,
        ))
        .with_state(running)
}

/// Refuses every request that does not present a usable token.
///
/// One layer rather than a check per handler, because a real server refuses
/// before it routes and a fake with nineteen copies of the check would be
/// nineteen chances to forget one (P7). Without it `verify_credential` and the
/// revoked-credential state it exists for were provable only by an injected
/// refusal — never by the condition itself.
async fn require_token(State(running): State<Running>, request: Request, next: Next) -> Response {
    let rendering = Rendering::of_headers(request.headers());
    let arguments = Arguments::parse(request.uri().query());
    let presented = request::token(request.headers(), &arguments);
    if !running.accepts_token(presented.as_deref()) {
        return rendering.refusal(StatusCode::UNAUTHORIZED, 1001, "Unauthorized");
    }
    next.run(request).await
}

/// `GET /` — the server root, which a real Plex answers only to a live token.
///
/// What it answers is a subset of a real root — every field here is one a real
/// server sends, because a fake that claimed more would put a shape in the
/// contract test that no Plex produces.
async fn root(State(running): State<Running>, rendering: Rendering) -> Result<Answer, Response> {
    if let Some(refusal) = running.gate(FakeOperation::Root).await {
        return Err(refusal);
    }
    let world = running.world();
    Ok(rendering.answer(
        shape::container()
            .text("machineIdentifier", world.machine_identifier.clone())
            .text("version", world.version.clone())
            .text("friendlyName", world.friendly_name.clone())
            .flag("myPlex", true)
            .text("platform", "Linux"),
    ))
}

/// `GET /identity`.
async fn identity(
    State(running): State<Running>,
    rendering: Rendering,
) -> Result<Answer, Response> {
    if let Some(refusal) = running.gate(FakeOperation::Identity).await {
        return Err(refusal);
    }
    let world = running.world();
    Ok(rendering.answer(
        Element::named("MediaContainer")
            .number("size", 0_i64)
            .flag("claimed", true)
            .text("machineIdentifier", world.machine_identifier.clone())
            .text("version", world.version.clone()),
    ))
}

/// `GET /library/sections`.
async fn sections(
    State(running): State<Running>,
    rendering: Rendering,
) -> Result<Answer, Response> {
    if let Some(refusal) = running.gate(FakeOperation::Sections).await {
        return Err(refusal);
    }
    let world = running.world();
    let directory: Vec<Element> = world.libraries.iter().map(shape::section).collect();
    Ok(rendering.answer(
        shape::container()
            .number("size", i64::try_from(directory.len()).unwrap_or(i64::MAX))
            .text("title1", "Plex Library")
            .children(directory),
    ))
}
