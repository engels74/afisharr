// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The write half of the contract: one collection, created, written, deleted.
//!
//! Split out of `contract.rs` under §24.6's file limit, and it splits along the
//! seam the test already had: the reads are compared call by call, and the
//! writes are one sequence against one collection that must not survive it.
//!
//! **Nothing here may panic between the create and the delete.** Every step
//! reports its failure and the sequence unwinds to the delete, because this
//! runs on somebody's real Plex and a scratch collection left behind is the one
//! thing these tests may not do (P2). The assertions run after the cleanup.

use afisharr_plex::{
    hubs::{HubIdentifier, HubVisibility},
    libraries::{ItemKind, ItemQuery, RatingKey, SectionKey, Window},
    server::{MachineIdentifier, PlexServerClient},
};
use serde_json::Value;

use crate::real::{self, Surface};

/// Creates a collection, exercises every write against it, and deletes it.
///
/// Every answer is kept, named as the lane reports it. The delete runs whatever
/// the rest did, because this runs on somebody's real Plex (P2).
pub async fn cycle(client: &PlexServerClient, surface: &Surface) -> Vec<(&'static str, Value)> {
    let mut answers = Vec::new();
    let title = real::scratch("write cycle");
    let identity = client
        .identity()
        .await
        .expect("the server must name itself before anything is written to it");
    let server = MachineIdentifier::new(identity.machine_identifier.as_str());

    let created = client
        .create_collection(
            &surface.section,
            ItemKind::Movie,
            &title,
            &server,
            std::slice::from_ref(&surface.item),
        )
        .await
        .expect("POST /library/collections must answer");

    let outcome = exercise(client, surface, &created.rating_key, &server, &mut answers).await;

    client
        .delete_collection(&created.rating_key)
        .await
        .expect("the collection this test created must be removable");
    let gone = client
        .collections(&surface.section)
        .await
        .expect("the collection list must answer");
    assert!(
        !gone.iter().any(|row| row.title == title),
        "nothing this test created may be left behind"
    );

    if let Err(failure) = outcome {
        panic!("{failure}");
    }
    answers
}

/// Reads what a real server answers to an edit, which is **Q-016**.
///
/// Read off the wire before anything interprets it. [`edit_at`] reduces the
/// answer to a count and reports every other shape as
/// `ServerError::Incomplete`, so the shape itself has to be read here or the
/// lane can only ever report "the edit failed" for the one question that would
/// tell somebody why.
///
/// Sent against the scratch collection this test deletes, and it sets the same
/// summary the typed edit sets straight afterwards, so nothing on the operator's
/// server is left carrying a value this probe invented (P2).
///
/// Answers the shape in words, for the messages that quote it.
///
/// [`edit_at`]: afisharr_plex::server::PlexServerClient
async fn answer_q016(
    client: &PlexServerClient,
    section: &SectionKey,
    collection: &RatingKey,
    answers: &mut Vec<(&'static str, Value)>,
) -> Result<String, String> {
    let written = real::try_raw_write(
        client,
        &format!("library/sections/{section}/all"),
        &[
            ("type".to_owned(), "18".to_owned()),
            ("id".to_owned(), collection.to_string()),
            (
                "summary.value".to_owned(),
                "Written by the Afisharr contract test.".to_owned(),
            ),
        ],
    )
    .await?;
    let shape = written.shape();
    answers.push(("PUT /library/sections/{key}/all", written.captured()));
    if !written.is_a_count() {
        return Err(format!(
            "Q-016 is answered, and against this build: a real server answered {shape} to \
             PUT /library/sections/{{id}}/all. `edits.rs::edit_at` reads that as \
             ServerError::Incomplete, so every edit this build makes — a collection edit, a \
             label edit, a sort-title write — is reported to the operator as a failure for a \
             write that landed. Change `edit_at` to read a sizeless answer as success, and \
             tick Task 2.1.7 subtask 6 with this capture."
        ));
    }
    Ok(shape)
}

/// Puts one collection into the ordering space, and names the row it got.
///
/// The call a `PUT` cannot stand in for: the collection has no manage row until
/// this runs.
///
/// **One axis set, not none.** A reference client only ever promotes by turning
/// an axis on — `promoteHome()` is `updateVisibility(home=True)`
/// (`plexapi/library.py:3122-3129`) — so a promotion with all three off is a
/// request nothing in reach has evidence about, and a row asserted after one
/// would be an assumption about how a real server answers it. What Afisharr
/// does is promote, so that is what the lane exercises.
async fn promote(
    client: &PlexServerClient,
    section: &SectionKey,
    collection: &RatingKey,
    answers: &mut Vec<(&'static str, Value)>,
) -> Result<HubIdentifier, String> {
    let visible = HubVisibility {
        own_home: true,
        ..HubVisibility::default()
    };
    let before = client
        .hubs(section)
        .await
        .map_err(|error| format!("the manage endpoint must answer: {error}"))?;
    client
        .set_collection_visibility(section, &before, collection, visible)
        .await
        .map_err(|error| format!("promotion must answer: {error}"))?;
    answers.push((
        "GET /hubs/sections/{key}/manage?metadataItemId",
        real::try_raw(
            client,
            &format!("hubs/sections/{section}/manage"),
            &[("metadataItemId".to_owned(), collection.to_string())],
        )
        .await?,
    ));

    let after = client
        .hubs(section)
        .await
        .map_err(|error| format!("the manage endpoint must answer: {error}"))?;
    let row = after
        .row_for(collection)
        .ok_or_else(|| "a promoted collection must have a manage row".to_owned())?;
    let identifier = row.identifier.clone();
    // The reading `names_collection` rests on, checked against the server that
    // composed it. A reference client synthesises this exact identifier for an
    // unpromoted collection and then reloads the promoted row by it
    // (`plexapi/collection.py:212`, `plexapi/library.py:3049-3052`), so a
    // server that composes it differently breaks the match every collection's
    // position depends on — silently, because the row is simply never found.
    let expected = format!("custom.collection.{section}.{collection}");
    if identifier.as_str() != expected {
        return Err(format!(
            "a promoted collection's manage row is named {identifier} and this build matches it \
             by {expected} (`hubs/record.rs::names_collection`), so no row would ever be found \
             for it"
        ));
    }
    Ok(identifier)
}

/// The writes themselves, so a failure in one still reaches the delete.
async fn exercise(
    client: &PlexServerClient,
    surface: &Surface,
    collection: &RatingKey,
    server: &MachineIdentifier,
    answers: &mut Vec<(&'static str, Value)>,
) -> Result<(), String> {
    let section = &surface.section;

    // The *read back* of what the create made, not the create's own answer:
    // `try_raw` can only issue a `GET`, so the `POST` body itself is still
    // uncompared. Named for what it is, because a capture filed under the
    // wrong call is worse than no capture at all. Reported rather than
    // panicked, like every other step here: a panic between the create and the
    // delete leaves the scratch collection on somebody's real Plex (P2).
    answers.push((
        "GET /library/collections/{key}/children",
        real::try_raw(
            client,
            &format!("library/collections/{collection}/children"),
            &ItemQuery::new(Window::first(20)).pairs(),
        )
        .await?,
    ));

    let q016 = answer_q016(client, section, collection, answers).await?;

    let edit = afisharr_plex::collections::CollectionEdit {
        sort: Some(afisharr_plex::collections::CollectionSort::Custom),
        summary: Some("Written by the Afisharr contract test.".to_owned()),
        ..afisharr_plex::collections::CollectionEdit::default()
    };
    client
        .edit_collection(section, collection, &edit)
        .await
        .map_err(|error| {
            format!(
                "the collection edit must answer: {error} (the same write read raw answered \
                 {q016}, which is Q-016)"
            )
        })?;

    client
        .add_collection_items(collection, server, std::slice::from_ref(&surface.item))
        .await
        .map_err(|error| format!("adding an item must answer: {error}"))?;

    let members = client
        .collection_items(collection, &ItemQuery::new(Window::first(20)))
        .await
        .map_err(|error| format!("the collection's children must answer: {error}"))?;
    if let Some(first) = members.items.first() {
        client
            .move_collection_item(
                collection,
                &first.rating_key,
                &afisharr_plex::collections::MoveTarget::ToFront,
            )
            .await
            .map_err(|error| format!("a move must answer: {error}"))?;
    }

    let identifier = promote(client, section, collection, answers).await?;
    client
        .move_hub(section, &identifier, &afisharr_plex::hubs::HubMove::ToFront)
        .await
        .map_err(|error| format!("a hub move must answer: {error}"))?;

    for item in members.items.iter().take(1) {
        client
            .remove_collection_item(collection, &item.rating_key)
            .await
            .map_err(|error| format!("removing an item must answer: {error}"))?;
    }
    Ok(())
}
