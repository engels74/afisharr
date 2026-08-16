// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The calls that read and write the ordering space.
//!
//! A collection is in the library the moment it is created and in the ordering
//! space only once something promotes it (`plexapi/collection.py:207-215`).
//! That is two states, and the fake used to have one: every collection was in
//! the manage answer from birth, so a promotion path that never promoted
//! anything passed.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Response,
};

use crate::fake::{
    element::Element,
    negotiation::{Answer, Rendering},
    plan::FakeOperation,
    populate::hub_identifier,
    request::Arguments,
    routes::Running,
    shape,
    state::FakeHub,
};

/// `GET /hubs/sections/{key}/manage`, with or without `metadataItemId`.
///
/// With one, a real server answers the single row for that collection or none
/// at all (`plexapi/collection.py:208`) — and "none" is how a client learns the
/// collection has never been promoted, which is a different fact from the
/// collection not existing.
pub(crate) async fn hubs(
    State(running): State<Running>,
    Path(key): Path<String>,
    rendering: Rendering,
    arguments: Arguments,
) -> Result<Answer, Response> {
    if let Some(refusal) = running.gate(FakeOperation::Hubs, rendering).await {
        return Err(refusal);
    }
    let wanted = arguments.first("metadataItemId").map(str::to_owned);
    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };
    let rows: Vec<Element> = library
        .hubs
        .iter()
        .filter(|hub| match &wanted {
            None => true,
            Some(wanted) => hub.rating_key.as_deref() == Some(wanted.as_str()),
        })
        .map(shape::hub)
        .collect();
    Ok(rendering.answer(
        shape::container()
            .number("size", i64::try_from(rows.len()).unwrap_or(i64::MAX))
            .text("librarySectionID", library.key.clone())
            .children(rows),
    ))
}

/// `POST /hubs/sections/{key}/manage` — how a collection enters the space.
///
/// The call a client makes when the manage answer carries no row for the
/// collection yet (`plexapi/library.py:3114-3117`). Without it there is no
/// promotion path at all: a `PUT` to `/manage/{identifier}` addresses a row a
/// real server does not have, answers 200, and changes nothing.
pub(crate) async fn promote(
    State(running): State<Running>,
    Path(key): Path<String>,
    rendering: Rendering,
    arguments: Arguments,
) -> Result<Answer, Response> {
    if let Some(refusal) = running.gate(FakeOperation::PromoteHub, rendering).await {
        return Err(refusal);
    }
    let Some(collection_key) = arguments.first("metadataItemId").map(str::to_owned) else {
        return Err(rendering.refusal(StatusCode::BAD_REQUEST, 1002, "Bad Request"));
    };
    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };
    let Some(title) = library
        .collections
        .iter()
        .find(|candidate| candidate.rating_key == collection_key)
        .map(|collection| collection.title.clone())
    else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };

    // Promoting a collection that already has a row writes that row rather than
    // adding a second: one collection is one row, and two would be a space no
    // real server answers with.
    let existing = library
        .hubs
        .iter()
        .position(|hub| hub.rating_key.as_deref() == Some(collection_key.as_str()));
    let at = existing.unwrap_or_else(|| {
        library.hubs.push(FakeHub {
            identifier: hub_identifier(&library.key, &collection_key),
            title,
            rating_key: Some(collection_key.clone()),
            deletable: true,
            own_home: false,
            shared_home: false,
            recommended: false,
        });
        library.hubs.len() - 1
    });
    let row = &mut library.hubs[at];
    write_axes(row, &arguments);
    let body = shape::hub(row);
    Ok(rendering.answer(shape::container().number("size", 1_i64).child(body)))
}

/// `PUT /hubs/sections/{key}/manage/{hub}/move`.
///
/// Answers 200 whether or not the order changed, for the reason the collection
/// move does: the silent no-op past the precision budget is the behaviour, and
/// only a verification read tells the two apart (§15.3).
pub(crate) async fn move_hub(
    State(running): State<Running>,
    Path((key, hub)): Path<(String, String)>,
    rendering: Rendering,
    arguments: Arguments,
) -> Result<Answer, Response> {
    if let Some(refusal) = running.gate(FakeOperation::MoveHub, rendering).await {
        return Err(refusal);
    }
    let after = arguments.first("after").map(str::to_owned);
    let mut world = running.world();
    if let Some(library) = world.library(&key) {
        library.move_hub(&hub, after.as_deref());
    }
    Ok(rendering.answer(shape::container()))
}

/// `PUT /hubs/sections/{key}/manage/{hub}`.
pub(crate) async fn set_visibility(
    State(running): State<Running>,
    Path((key, hub)): Path<(String, String)>,
    rendering: Rendering,
    arguments: Arguments,
) -> Result<Answer, Response> {
    if let Some(refusal) = running
        .gate(FakeOperation::SetHubVisibility, rendering)
        .await
    {
        return Err(refusal);
    }
    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };
    let Some(row) = library
        .hubs
        .iter_mut()
        .find(|candidate| candidate.identifier == hub)
    else {
        // A real server has no row here until something promotes the
        // collection, and answering as though it wrote one is what let a
        // promotion path that never promoted anything pass.
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };
    write_axes(row, &arguments);
    Ok(rendering.answer(shape::container().number("size", 1_i64)))
}

/// `DELETE /hubs/sections/{key}/manage/{hub}` — one row leaves the space.
pub(crate) async fn remove_hub(
    State(running): State<Running>,
    Path((key, hub)): Path<(String, String)>,
    rendering: Rendering,
) -> Result<Answer, Response> {
    if let Some(refusal) = running.gate(FakeOperation::RemoveHub, rendering).await {
        return Err(refusal);
    }
    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };
    // One of Plex's own rows says `deletable="0"` and stays whatever it is
    // asked (§15.1). The refusal is silent and answers 200 like everything else
    // here: an invented status code would be this fake claiming something no
    // real server was seen to send.
    library
        .hubs
        .retain(|candidate| candidate.identifier != hub || !candidate.deletable);
    Ok(rendering.answer(shape::container()))
}

/// `DELETE /hubs/sections/{key}/manage` — every removable row leaves.
pub(crate) async fn remove_every_hub(
    State(running): State<Running>,
    Path(key): Path<String>,
    rendering: Rendering,
) -> Result<Answer, Response> {
    if let Some(refusal) = running.gate(FakeOperation::RemoveHub, rendering).await {
        return Err(refusal);
    }
    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };
    library.hubs.retain(|candidate| !candidate.deletable);
    Ok(rendering.answer(shape::container()))
}

/// Writes the three visibility axes one request names.
///
/// Each axis written only when the request names it, exactly as Plex does.
/// Defaulting the two a caller omitted to `false` would hide a caller that
/// forgot one, and hide it as an accidental unpromote (§15.5).
fn write_axes(row: &mut FakeHub, arguments: &Arguments) {
    // A row that cannot be removed cannot be unpromoted either, which is what
    // makes it an anchor to work around rather than a participant to move
    // (§15.1). A fake that let it be unpromoted would pass a planner reaching
    // for a recovery move the real surface does not offer, right up to the
    // first real server.
    let anchored = !row.deletable;
    for (name, axis) in [
        ("promotedToOwnHome", &mut row.own_home),
        ("promotedToSharedHome", &mut row.shared_home),
        ("promotedToRecommended", &mut row.recommended),
    ] {
        let Some(value) = arguments.first(name) else {
            continue;
        };
        let promoted = value != "0";
        if promoted || !anchored {
            *axis = promoted;
        }
    }
}
