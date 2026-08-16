// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The calls that read or write one library's items.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Response,
};

use crate::fake::{
    choices, edit,
    element::Element,
    negotiation::{Answer, Rendering},
    plan::FakeOperation,
    request::{Arguments, Paging},
    routes::Running,
    search, shape,
    vocabulary::{self, plex_type},
};

/// `GET /library/sections/{key}/all`.
///
/// One endpoint for three questions, exactly as Plex serves it: with
/// `includeMeta=1` it describes its own filter vocabulary, with `type=18` it
/// lists collections, and otherwise it lists items. Splitting them here would
/// be a fake with endpoints the real server does not have.
pub(crate) async fn items(
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
        FakeOperation::Items
    };
    if let Some(refusal) = running.gate(operation, rendering).await {
        return Err(refusal);
    }
    if !describing {
        running.note_fetch();
    }

    let detail = running.detail(&arguments);
    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };

    // Collections are items of type 18 on this endpoint, and answering movies
    // to a caller that asked for collections is a fake answering a different
    // question confidently (`plexapi/library.py:1666-1670`).
    let wants_collections = edit::libtype(&arguments) == Some(plex_type("collection"));
    let mut container = shape::library_container(library).text("title1", library.title.clone());
    let rows: Vec<Element> = if wants_collections {
        library
            .collections
            .iter()
            .map(|collection| shape::collection(collection, library))
            .collect()
    } else {
        search::select(&library.items, &arguments)
            .into_iter()
            .map(|item| shape::item(item, library, detail))
            .collect()
    };

    let total = rows.len();
    let page: Vec<Element> = rows
        .into_iter()
        .skip(paging.start)
        .take(paging.size)
        .collect();
    container = container
        .number("size", i64::try_from(page.len()).unwrap_or(i64::MAX))
        .number("totalSize", i64::try_from(total).unwrap_or(i64::MAX))
        .number("offset", i64::try_from(paging.start).unwrap_or(i64::MAX));
    if describing {
        container = container.child(vocabulary::describe(
            &key,
            vocabulary::libtypes_of(&library.kind),
            arguments.flag("includeAdvanced"),
        ));
    }
    Ok(rendering.answer(container.children(page)))
}

/// `GET /library/metadata/{key}` — one item, or one collection.
///
/// Both, because a real server serves both here: a collection's own `key` is
/// `/library/metadata/{ratingKey}/children` with the suffix stripped, which is
/// how every client reloads one.
pub(crate) async fn item(
    State(running): State<Running>,
    Path(key): Path<String>,
    rendering: Rendering,
    arguments: Arguments,
) -> Result<Answer, Response> {
    if let Some(refusal) = running.gate(FakeOperation::Item, rendering).await {
        return Err(refusal);
    }
    let detail = running.detail(&arguments);
    let mut world = running.world();

    if let Some(library) = world.library_of_item(&key) {
        let container = shape::library_container(library).number("size", 1_i64);
        let row = library
            .items
            .iter()
            .find(|candidate| candidate.rating_key == key)
            .map(|found| shape::item(found, library, detail));
        return Ok(rendering.answer(container.children(row)));
    }
    if let Some(library) = world.library_of_collection(&key) {
        let container = shape::library_container(library).number("size", 1_i64);
        let row = library
            .collections
            .iter()
            .find(|candidate| candidate.rating_key == key)
            .map(|found| shape::collection(found, library));
        return Ok(rendering.answer(container.children(row)));
    }

    // A real server refuses a key it does not hold, and a rebound rating key is
    // exactly that case (`I-ID-1`). The empty container is the other shape a
    // client must survive, and a scenario chooses it — asserting one of the two
    // is what left half the clients in the world untested.
    if running.missing_item_answers_empty() {
        return Ok(rendering.answer(shape::container()));
    }
    Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"))
}

/// `PUT /library/sections/{key}/all` — the one edit endpoint, over every
/// libtype.
///
/// What is edited is whatever `id` names, at the libtype `type` names
/// (`plexapi/library.py:1743-1755`). Deciding from the presence of a `label`
/// argument, as this used to, made an item's sort title unwritable.
pub(crate) async fn edit(
    State(running): State<Running>,
    Path(key): Path<String>,
    rendering: Rendering,
    arguments: Arguments,
) -> Result<Answer, Response> {
    let touches_labels = arguments
        .pairs()
        .iter()
        .any(|(name, _)| name.starts_with("label"));
    // The operation an injection is aimed at follows what the caller meant, so
    // a scenario failing label edits does not land on a title edit.
    let operation = if touches_labels {
        FakeOperation::EditLabels
    } else {
        FakeOperation::EditCollection
    };
    if let Some(refusal) = running.gate(operation, rendering).await {
        return Err(refusal);
    }

    let ids = edit::targets(&arguments);
    let collections = edit::libtype(&arguments) == Some(plex_type("collection"));
    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };

    // Counted, not assumed: an edit naming an id this server does not hold
    // wrote nothing, and answering `size: 1` regardless is how a caller comes
    // to believe a write it never got.
    let mut written = 0_i64;
    for id in ids {
        let applied = if collections {
            library
                .collection(&id)
                .is_some_and(|target| edit::apply_to_collection(target, &arguments))
        } else {
            library
                .item(&id)
                .is_some_and(|target| edit::apply_to_item(target, &arguments))
        };
        written += i64::from(applied);
    }
    Ok(rendering.answer(shape::container().number("size", written)))
}

/// `GET /library/sections/{key}/{filter}` — a filter's enumerated choices.
pub(crate) async fn choices(
    State(running): State<Running>,
    Path((key, filter)): Path<(String, String)>,
    rendering: Rendering,
) -> Result<Answer, Response> {
    if let Some(refusal) = running.gate(FakeOperation::FilterChoices, rendering).await {
        return Err(refusal);
    }
    let mut world = running.world();
    let Some(library) = world.library(&key) else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };
    // Only the filter that declared a choice endpoint has choices. Answering a
    // list for one that did not would let a client that ignores the declaration
    // pass here and fail against a real server.
    let Some(listed) = choices::choices(library, &filter) else {
        return Err(rendering.refusal(StatusCode::NOT_FOUND, 1000, "Not Found"));
    };
    Ok(rendering.answer(
        shape::library_container(library)
            .number("size", i64::try_from(listed.len()).unwrap_or(i64::MAX))
            .children(listed),
    ))
}

/// `POST /library/metadata/{key}/posters`.
pub(crate) async fn upload_poster(
    State(running): State<Running>,
    Path(key): Path<String>,
    rendering: Rendering,
    body: axum::body::Bytes,
) -> Result<Answer, Response> {
    if let Some(refusal) = running.gate(FakeOperation::UploadPoster, rendering).await {
        return Err(refusal);
    }
    let mut world = running.world();
    let mut uploaded = 0_i64;
    for library in &mut world.libraries {
        if let Some(item) = library.item(&key) {
            // Keyed on the size so a test can tell one upload from another
            // without the fake storing megabytes of image it will never serve.
            item.thumb = format!("/library/metadata/{key}/thumb/upload-{}", body.len());
            uploaded += 1;
        }
    }
    Ok(rendering.answer(shape::container().number("size", uploaded)))
}
