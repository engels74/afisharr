// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The calls that read and write the ordering space.

use axum::{
    Json,
    extract::{Path, Query, State},
    response::Response,
};
use serde_json::{Value, json};

use crate::fake::{
    json as shape,
    plan::FakeOperation,
    routes::{Params, Running},
};

/// `GET /hubs/sections/{key}/manage`.
pub(crate) async fn hubs(
    State(running): State<Running>,
    Path(key): Path<String>,
) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::Hubs).await {
        return Err(refusal);
    }
    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Ok(Json(shape::container(&json!({ "size": 0 }))));
    };
    let hubs: Vec<Value> = library.hubs.iter().map(shape::hub).collect();
    Ok(Json(shape::container(&json!({
        "size": hubs.len(),
        "Hub": hubs,
    }))))
}

/// `PUT /hubs/sections/{key}/manage/{hub}/move`.
///
/// Answers 200 whether or not the order changed, for the reason the collection
/// move does: the silent no-op past the precision budget is the behaviour, and
/// only a verification read tells the two apart (§15.3).
pub(crate) async fn move_hub(
    State(running): State<Running>,
    Path((key, hub)): Path<(String, String)>,
    Query(params): Query<Params>,
) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::MoveHub).await {
        return Err(refusal);
    }
    let after = params.get("after").cloned();
    let mut world = running.world();
    if let Some(library) = world.library(&key) {
        library.move_hub(&hub, after.as_deref());
    }
    Ok(Json(shape::container(&json!({ "size": 0 }))))
}

/// `PUT /hubs/sections/{key}/manage/{hub}`.
pub(crate) async fn set_visibility(
    State(running): State<Running>,
    Path((key, hub)): Path<(String, String)>,
    Query(params): Query<Params>,
) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::SetHubVisibility).await {
        return Err(refusal);
    }
    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Ok(Json(shape::container(&json!({ "size": 0 }))));
    };
    let Some(row) = library
        .hubs
        .iter_mut()
        .find(|candidate| candidate.identifier == hub)
    else {
        return Ok(Json(shape::container(&json!({ "size": 0 }))));
    };
    // Each axis written only when the request names it, exactly as Plex does.
    // Defaulting the two a caller omitted to `false` would hide a caller that
    // forgot one, and hide it as an accidental unpromote (§15.5).
    if let Some(value) = params.get("promotedToOwnHome") {
        row.own_home = value != "0";
    }
    if let Some(value) = params.get("promotedToSharedHome") {
        row.shared_home = value != "0";
    }
    if let Some(value) = params.get("promotedToRecommended") {
        row.recommended = value != "0";
    }
    Ok(Json(shape::container(&json!({ "size": 1 }))))
}
