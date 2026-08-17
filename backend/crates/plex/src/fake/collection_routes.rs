// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The calls that read and write collections.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Response,
};

use crate::fake::{
    element::Element,
    negotiation::{Answer, Rendering},
    plan::FakeOperation,
    request::{Arguments, Paging},
    routes::Running,
    search, shape,
    state::FakeCollection,
    vocabulary,
};

/// `GET /library/sections/{key}/collections`.
///
/// Answers `includeMeta=1` as well as `/all` does, because a client loads half
/// its filter vocabulary from here (`plexapi/library.py:890-899`): this is
/// where the `collection` libtype's filters come from, and a client that got no
/// `Meta` here could not filter collections at all.
pub(crate) async fn collections(
    State(running): State<Running>,
    Path(key): Path<String>,
    rendering: Rendering,
    arguments: Arguments,
    paging: Paging,
) -> Result<Answer, Response> {
    let describing = arguments.flag("includeMeta");
    let operation = if describing {
        FakeOperation::Vocabulary
    } else {
        FakeOperation::Collections
    };
    if let Some(refusal) = running.gate(operation, rendering).await {
        return Err(refusal);
    }
    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };
    // The same selection `/all?type=18` makes, because it is the same question
    // asked at the other endpoint a client lists collections from. A filter
    // honoured at one and ignored at the other is a fake that answers
    // differently depending on which route the caller happened to pick.
    let rows: Vec<Element> = search::select(&library.collections, &arguments)
        .into_iter()
        .map(|collection| shape::collection(collection, library))
        .collect();
    let total = rows.len();
    let page: Vec<Element> = rows
        .into_iter()
        .skip(paging.start)
        .take(paging.size)
        .collect();
    let mut container = shape::library_container(library)
        .number("size", i64::try_from(page.len()).unwrap_or(i64::MAX))
        .number("totalSize", i64::try_from(total).unwrap_or(i64::MAX))
        .text("title1", library.title.clone())
        .text("viewGroup", "collection");
    if describing {
        container = container.child(vocabulary::describe(
            &key,
            &["collection"],
            arguments.flag("includeAdvanced"),
        ));
    }
    Ok(rendering.answer(container.children(page)))
}

/// `POST /library/collections`.
///
/// The created collection is in the library and *not* in the ordering space: a
/// real server answers no manage row for a collection nothing has promoted
/// (`plexapi/collection.py:207-215`). The row this used to push made a broken
/// promotion path pass, because `set_hub_visibility` found a row to write that
/// a real server would not have had.
pub(crate) async fn create_collection(
    State(running): State<Running>,
    rendering: Rendering,
    arguments: Arguments,
) -> Result<Answer, Response> {
    if let Some(refusal) = running
        .gate(FakeOperation::CreateCollection, rendering)
        .await
    {
        return Err(refusal);
    }
    let title = arguments.first("title").unwrap_or_default().to_owned();
    let section = arguments.first("sectionId").unwrap_or_default().to_owned();
    let items = rating_keys(arguments.first("uri"));
    let smart = arguments.first("smart").is_some_and(|value| value != "0");
    let budget = running.move_budget();

    let mut world = running.world();
    let Some(library) = world.library(&section) else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
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
        summary: None,
        subtype: library.kind.clone(),
        mode: -1,
        // Release order, which is where a real server starts one
        // (`plexapi/collection.py:73`). Custom order is a thing Afisharr must
        // switch on, and a fake that started there tests nothing.
        sort: 0,
        smart,
        labels: Vec::new(),
        labels_locked: false,
        items,
        moves_left: budget,
    };
    let body = shape::collection(&collection, library);
    library.collections.push(collection);
    Ok(rendering.answer(
        shape::library_container(library)
            .number("size", 1_i64)
            .child(body),
    ))
}

/// `DELETE /library/collections/{key}` and `DELETE /library/metadata/{key}`.
pub(crate) async fn delete_collection(
    State(running): State<Running>,
    Path(key): Path<String>,
    rendering: Rendering,
) -> Result<Answer, Response> {
    if let Some(refusal) = running
        .gate(FakeOperation::DeleteCollection, rendering)
        .await
    {
        return Err(refusal);
    }
    let mut world = running.world();
    for library in &mut world.libraries {
        library
            .collections
            .retain(|collection| collection.rating_key != key);
        // And out of the ordering space, if something had promoted it there.
        library
            .hubs
            .retain(|hub| hub.rating_key.as_deref() != Some(key.as_str()));
    }
    Ok(rendering.answer(shape::container()))
}

/// `GET /library/collections/{key}/children` and
/// `GET /library/metadata/{key}/children`.
pub(crate) async fn collection_items(
    State(running): State<Running>,
    Path(key): Path<String>,
    rendering: Rendering,
    arguments: Arguments,
    paging: Paging,
) -> Result<Answer, Response> {
    if let Some(refusal) = running
        .gate(FakeOperation::CollectionItems, rendering)
        .await
    {
        return Err(refusal);
    }
    let detail = running.detail(&arguments);
    let mut world = running.world();
    let Some(library) = world.library_of_collection(&key) else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };
    let Some(collection) = library
        .collections
        .iter()
        .find(|candidate| candidate.rating_key == key)
    else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };
    // Read in the collection's own order, which is the order a verification
    // read has to see: an answer sorted by anything else would hide exactly the
    // no-op move §15.3 describes.
    let ordered: Vec<Element> = collection
        .items
        .iter()
        .filter_map(|rating_key| {
            library
                .items
                .iter()
                .find(|item| &item.rating_key == rating_key)
        })
        .map(|item| shape::item(item, library, detail))
        .collect();
    let total = ordered.len();
    let page: Vec<Element> = ordered
        .into_iter()
        .skip(paging.start)
        .take(paging.size)
        .collect();
    Ok(rendering.answer(
        shape::library_container(library)
            .number("size", i64::try_from(page.len()).unwrap_or(i64::MAX))
            .number("totalSize", i64::try_from(total).unwrap_or(i64::MAX))
            .text("title2", collection.title.clone())
            .children(page),
    ))
}

/// `PUT /library/collections/{key}/items` and `PUT /library/metadata/{key}/items`.
pub(crate) async fn add_items(
    State(running): State<Running>,
    Path(key): Path<String>,
    rendering: Rendering,
    arguments: Arguments,
) -> Result<Answer, Response> {
    if let Some(refusal) = running
        .gate(FakeOperation::AddCollectionItems, rendering)
        .await
    {
        return Err(refusal);
    }
    let adding = rating_keys(arguments.first("uri"));
    let mut world = running.world();
    let Some(library) = world.library_of_collection(&key) else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };
    let mut added = 0_i64;
    if let Some(collection) = library.collection(&key) {
        for rating_key in adding {
            if !collection.items.contains(&rating_key) {
                collection.items.push(rating_key);
                added += 1;
            }
        }
    }
    Ok(rendering.answer(shape::container().number("size", added)))
}

/// `DELETE /library/collections/{key}/items/{item}` and its metadata twin.
pub(crate) async fn remove_item(
    State(running): State<Running>,
    Path((key, item)): Path<(String, String)>,
    rendering: Rendering,
) -> Result<Answer, Response> {
    if let Some(refusal) = running
        .gate(FakeOperation::RemoveCollectionItem, rendering)
        .await
    {
        return Err(refusal);
    }
    let mut world = running.world();
    if let Some(library) = world.library_of_collection(&key)
        && let Some(collection) = library.collection(&key)
    {
        collection.items.retain(|candidate| candidate != &item);
    }
    Ok(rendering.answer(shape::container()))
}

/// `PUT /library/collections/{key}/items/{item}/move` and its metadata twin.
///
/// Answers 200 whether or not the order changed. That is the misbehaviour, not
/// an oversight: past the precision budget a real server reports success and
/// leaves the sequence alone, and a fake that reported the difference would let
/// a planner skip the verification read §15.3 requires.
pub(crate) async fn move_item(
    State(running): State<Running>,
    Path((key, item)): Path<(String, String)>,
    rendering: Rendering,
    arguments: Arguments,
) -> Result<Answer, Response> {
    if let Some(refusal) = running
        .gate(FakeOperation::MoveCollectionItem, rendering)
        .await
    {
        return Err(refusal);
    }
    let after = arguments.first("after").map(str::to_owned);
    let mut world = running.world();
    if let Some(library) = world.library_of_collection(&key) {
        library.move_collection_item(&key, &item, after.as_deref());
    }
    Ok(rendering.answer(shape::container()))
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
