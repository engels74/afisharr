// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The calls that read and write collections.

use axum::{
    Json,
    extract::{Path, Query, State},
    response::Response,
};
use serde_json::{Value, json};

use crate::fake::{
    json as shape,
    plan::FakeOperation,
    routes::{Params, Running, window},
    state::{FakeCollection, FakeHub},
};

/// `GET /library/sections/{key}/collections`.
pub(crate) async fn collections(
    State(running): State<Running>,
    Path(key): Path<String>,
) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::Collections).await {
        return Err(refusal);
    }
    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Ok(Json(shape::container(&json!({ "size": 0 }))));
    };
    let metadata: Vec<Value> = library.collections.iter().map(shape::collection).collect();
    Ok(Json(shape::container(&json!({
        "size": metadata.len(),
        "Metadata": metadata,
    }))))
}

/// `POST /library/collections`.
pub(crate) async fn create_collection(
    State(running): State<Running>,
    Query(params): Query<Params>,
) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::CreateCollection).await {
        return Err(refusal);
    }
    let title = params.get("title").cloned().unwrap_or_default();
    let section = params.get("sectionId").cloned().unwrap_or_default();
    let items = rating_keys(params.get("uri").map(String::as_str));

    let mut world = running.world();
    let Some(library) = world.library(&section) else {
        return Ok(Json(shape::container(&json!({ "size": 0 }))));
    };
    // Derived from the count, then advanced past anything that already holds
    // it: a create/delete/create run would otherwise mint a key a live
    // collection already answers to, and every later call addressing it would
    // land on whichever of the two comes first in the list.
    let mut suffix = 6000 + library.collections.len();
    while library
        .collections
        .iter()
        .any(|candidate| candidate.rating_key == format!("{section}{suffix}"))
    {
        suffix += 1;
    }
    let collection = FakeCollection {
        rating_key: format!("{section}{suffix}"),
        title,
        sort_title: None,
        sort_title_locked: false,
        items,
    };
    let body = shape::collection(&collection);
    // And into the ordering space, because on a real server it is there the
    // moment it exists: `/hubs/sections/{key}/manage` lists every collection in
    // the library, promoted or not. `delete_collection` already takes the row
    // out again, and a create that did not put one in left the two halves
    // disagreeing — a collection created through the client never appeared in
    // the hub list, so `set_hub_visibility` and `move_hub` against it fell
    // through their lookups and answered 200 having changed nothing. A broken
    // promotion path passes against a fake like that.
    //
    // Hidden on all three surfaces, which is what a new collection is: in the
    // space to be ordered, on nobody's home screen until something promotes it.
    library.hubs.push(FakeHub {
        identifier: format!("collection.{}", collection.rating_key),
        title: collection.title.clone(),
        rating_key: Some(collection.rating_key.clone()),
        own_home: false,
        shared_home: false,
        recommended: false,
    });
    library.collections.push(collection);
    Ok(Json(shape::container(&json!({
        "size": 1,
        "Metadata": [body],
    }))))
}

/// `DELETE /library/collections/{key}`.
pub(crate) async fn delete_collection(
    State(running): State<Running>,
    Path(key): Path<String>,
) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::DeleteCollection).await {
        return Err(refusal);
    }
    let mut world = running.world();
    for library in &mut world.libraries {
        library
            .collections
            .retain(|collection| collection.rating_key != key);
        library
            .hubs
            .retain(|hub| hub.rating_key.as_deref() != Some(key.as_str()));
    }
    Ok(Json(shape::container(&json!({ "size": 0 }))))
}

/// `GET /library/collections/{key}/children`.
pub(crate) async fn collection_items(
    State(running): State<Running>,
    Path(key): Path<String>,
    Query(params): Query<Params>,
) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::CollectionItems).await {
        return Err(refusal);
    }
    let mut world = running.world();
    let Some(library) = world.library_of_collection(&key) else {
        return Ok(Json(shape::container(&json!({ "size": 0 }))));
    };
    let Some(collection) = library
        .collections
        .iter()
        .find(|candidate| candidate.rating_key == key)
    else {
        return Ok(Json(shape::container(&json!({ "size": 0 }))));
    };
    // Read in the collection's own order, which is the order a verification
    // read has to see: an answer sorted by anything else would hide exactly the
    // no-op move §15.3 describes.
    let ordered: Vec<Value> = collection
        .items
        .iter()
        .filter_map(|rating_key| {
            library
                .items
                .iter()
                .find(|item| &item.rating_key == rating_key)
        })
        .map(shape::item)
        .collect();
    let total = ordered.len();
    let (start, size) = window(&params);
    let page: Vec<Value> = ordered.into_iter().skip(start).take(size).collect();
    Ok(Json(shape::container(&json!({
        "size": page.len(),
        "totalSize": total,
        "Metadata": page,
    }))))
}

/// `PUT /library/collections/{key}/items`.
pub(crate) async fn add_items(
    State(running): State<Running>,
    Path(key): Path<String>,
    Query(params): Query<Params>,
) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::AddCollectionItems).await {
        return Err(refusal);
    }
    let adding = rating_keys(params.get("uri").map(String::as_str));
    let mut world = running.world();
    let Some(library) = world.library_of_collection(&key) else {
        return Ok(Json(shape::container(&json!({ "size": 0 }))));
    };
    if let Some(collection) = library
        .collections
        .iter_mut()
        .find(|candidate| candidate.rating_key == key)
    {
        for rating_key in adding {
            if !collection.items.contains(&rating_key) {
                collection.items.push(rating_key);
            }
        }
    }
    Ok(Json(shape::container(&json!({ "size": 1 }))))
}

/// `DELETE /library/collections/{key}/items/{item}`.
pub(crate) async fn remove_item(
    State(running): State<Running>,
    Path((key, item)): Path<(String, String)>,
) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::RemoveCollectionItem).await {
        return Err(refusal);
    }
    let mut world = running.world();
    if let Some(library) = world.library_of_collection(&key)
        && let Some(collection) = library
            .collections
            .iter_mut()
            .find(|candidate| candidate.rating_key == key)
    {
        collection.items.retain(|candidate| candidate != &item);
    }
    Ok(Json(shape::container(&json!({ "size": 0 }))))
}

/// `PUT /library/collections/{key}/items/{item}/move`.
///
/// Answers 200 whether or not the order changed. That is the misbehaviour, not
/// an oversight: past the precision budget a real server reports success and
/// leaves the sequence alone, and a fake that reported the difference would let
/// a planner skip the verification read §15.3 requires.
pub(crate) async fn move_item(
    State(running): State<Running>,
    Path((key, item)): Path<(String, String)>,
    Query(params): Query<Params>,
) -> Result<Json<Value>, Response> {
    if let Some(refusal) = running.gate(FakeOperation::MoveCollectionItem).await {
        return Err(refusal);
    }
    let after = params.get("after").cloned();
    let mut world = running.world();
    if let Some(library) = world.library_of_collection(&key) {
        library.move_collection_item(&key, &item, after.as_deref());
    }
    Ok(Json(shape::container(&json!({ "size": 0 }))))
}

/// The rating keys named by a `server://…` URI.
fn rating_keys(uri: Option<&str>) -> Vec<String> {
    uri.and_then(|uri| uri.rsplit_once("/library/metadata/"))
        .map(|(_, keys)| {
            keys.split(',')
                .filter(|key| !key.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_library_uri_names_the_keys_it_carries() {
        assert_eq!(
            rating_keys(Some(
                "server://abc/com.plexapp.plugins.library/library/metadata/1,2,3"
            )),
            ["1", "2", "3"]
        );
    }

    #[test]
    fn a_uri_naming_nothing_adds_nothing() {
        // The shape `library_uri` refuses to build. If it ever reached here, a
        // fake that read it as "every item" would hide the bug.
        assert!(rating_keys(None).is_empty());
        assert!(
            rating_keys(Some(
                "server://abc/com.plexapp.plugins.library/library/metadata/"
            ))
            .is_empty()
        );
    }
}
