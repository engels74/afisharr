// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The calls that read or write one library's items.

use axum::{
    Json,
    extract::{Path, Query, RawQuery, State},
    response::Response,
};
use serde_json::{Value, json};

use crate::fake::{
    json as shape,
    plan::FakeOperation,
    routes::{Params, Running, first, pairs, window},
    vocabulary,
};

/// `GET /library/sections/{key}/all`.
///
/// One endpoint for two questions, exactly as Plex serves it: with
/// `includeMeta=1` it describes its own filter vocabulary, and without it it
/// lists items. Splitting them here would be a fake with an endpoint the real
/// server does not have.
pub(crate) async fn items(
    State(running): State<Running>,
    Path(key): Path<String>,
    Query(params): Query<Params>,
) -> Result<Json<Value>, Response> {
    if params.get("includeMeta").is_some_and(|value| value == "1") {
        if let Some(refusal) = running.gate(FakeOperation::Vocabulary).await {
            return Err(refusal);
        }
        // Described for the type that was asked about, not always for movies:
        // a vocabulary answered under one libtype and read as another is a
        // discovery test asserting against a library it never queried.
        return Ok(Json(vocabulary::describe(
            &key,
            params.get("type").map(String::as_str),
        )));
    }

    if let Some(refusal) = running.gate(FakeOperation::Items).await {
        return Err(refusal);
    }
    running.note_fetch();

    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Ok(Json(shape::container(&json!({ "size": 0 }))));
    };
    let (start, size) = window(&params);
    let total = library.items.len();
    let page: Vec<Value> = library
        .items
        .iter()
        .skip(start)
        .take(size)
        .map(shape::item)
        .collect();
    Ok(Json(shape::container(&json!({
        "size": page.len(),
        "totalSize": total,
        "offset": start,
        "Metadata": page,
    }))))
}

/// `GET /library/metadata/{key}`.
pub(crate) async fn item(
    State(running): State<Running>,
    Path(key): Path<String>,
) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::Item).await {
        return Err(refusal);
    }
    let world = running.world();
    let found = world
        .libraries
        .iter()
        .flat_map(|library| library.items.iter())
        .find(|item| item.rating_key == key)
        .map(shape::item);
    // An answer with no item, not a 404: a rebound rating key is the case
    // `I-ID-1` is about, and a server that 404s and one that answers an empty
    // container are both shapes a client must survive.
    let body = match found {
        None => json!({ "size": 0 }),
        Some(item) => json!({ "size": 1, "Metadata": [item] }),
    };
    Ok(Json(shape::container(&body)))
}

/// `PUT /library/sections/{key}/all` — a collection edit, or a label edit.
///
/// One endpoint for both, as Plex serves it. Which one it is is decided by the
/// arguments, and the fake reports the same operation the client meant so an
/// injection aimed at labels does not land on a title edit.
pub(crate) async fn edit(
    State(running): State<Running>,
    Path(key): Path<String>,
    RawQuery(query): RawQuery,
) -> Result<Json<Value>, Response> {
    // Read as pairs rather than as a map: a removal is sent once per label
    // under one repeated key, and a map would honour the last of them.
    let params = pairs(query.as_deref());
    let labels = params.iter().any(|(name, _)| name.starts_with("label"));
    let operation = if labels {
        FakeOperation::EditLabels
    } else {
        FakeOperation::EditCollection
    };
    if let Some(refusal) = running.gate(operation).await {
        return Err(refusal);
    }

    let Some(id) = first(&params, "id").map(str::to_owned) else {
        return Ok(Json(shape::container(&json!({ "size": 0 }))));
    };
    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Ok(Json(shape::container(&json!({ "size": 0 }))));
    };

    if labels {
        apply_labels(library, &id, &params);
    } else {
        apply_collection_edit(library, &id, &params);
    }
    Ok(Json(shape::container(&json!({ "size": 1 }))))
}

/// Applies a label edit to one item.
fn apply_labels(
    library: &mut crate::fake::state::FakeLibrary,
    id: &str,
    params: &[(String, String)],
) {
    let Some(item) = library
        .items
        .iter_mut()
        .find(|candidate| candidate.rating_key == id)
    else {
        return;
    };
    for (name, value) in params {
        if name == "label[].tag.tag-" {
            item.labels.retain(|label| label != value);
        } else if name.starts_with("label[")
            && name.ends_with("].tag.tag")
            && !item.labels.iter().any(|label| label == value)
        {
            item.labels.push(value.clone());
        }
    }
}

/// Applies a title or sort-title edit to one collection.
fn apply_collection_edit(
    library: &mut crate::fake::state::FakeLibrary,
    id: &str,
    params: &[(String, String)],
) {
    let Some(collection) = library
        .collections
        .iter_mut()
        .find(|candidate| candidate.rating_key == id)
    else {
        return;
    };
    if let Some(title) = first(params, "title.value") {
        title.clone_into(&mut collection.title);
    }
    if let Some(sort_title) = first(params, "titleSort.value") {
        collection.sort_title = Some(sort_title.to_owned());
    }
    // Written whenever it is sent, including to `0`. A fake that only ever set
    // the lock would make a restore that forgot to clear it look correct
    // (`I-REV-3`).
    if let Some(locked) = first(params, "titleSort.locked") {
        collection.sort_title_locked = locked != "0";
    }
}

/// `GET /library/sections/{key}/{filter}` — a filter's enumerated choices.
pub(crate) async fn choices(
    State(running): State<Running>,
    Path((key, filter)): Path<(String, String)>,
) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::FilterChoices).await {
        return Err(refusal);
    }
    Ok(Json(vocabulary::choices(&key, &filter)))
}

/// `POST /library/metadata/{key}/posters`.
pub(crate) async fn upload_poster(
    State(running): State<Running>,
    Path(key): Path<String>,
    body: axum::body::Bytes,
) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::UploadPoster).await {
        return Err(refusal);
    }
    let mut world = running.world();
    for library in &mut world.libraries {
        if let Some(item) = library
            .items
            .iter_mut()
            .find(|candidate| candidate.rating_key == key)
        {
            // Keyed on the size so a test can tell one upload from another
            // without the fake storing megabytes of image it will never serve.
            item.thumb = format!("/library/metadata/{key}/thumb/upload-{}", body.len());
        }
    }
    Ok(Json(shape::container(&json!({ "size": 1 }))))
}
