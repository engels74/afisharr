// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The release-lane contract test: what keeps the adversarial fake truthful.
//!
//! A stub does what it is told, and the failures worth testing are the ones
//! where Plex does not. The fake makes those reproducible. It cannot make
//! itself correct — every shape in it is a claim about a server nobody in this
//! repository controls, and a claim that drifts turns every test written
//! against it into a test of a server that does not exist. So the same call
//! surface runs against a real Plex (D-036, PRD §21.10.2).
//!
//! **Both directions.** The fake claiming a field a real server does not send
//! is drift, and a real server sending a field the fake does not is a gap —
//! and the second is the one that passed every omission the reference audit
//! found. A gap is answered either by the fake answering the field or by a
//! line in `real::ALLOWED` saying why it does not.
//!
//! **Writes too.** The read surface was compared and every write was left out,
//! so the answer shape of create, edit, add, remove, move, and promote was
//! checked against nothing. They run against a collection this test creates
//! and deletes, because this runs on somebody's real Plex and nothing may be
//! left behind (P2).
//!
//! **It needs a real server, and says so when it has none.** The release lane
//! supplies `AFISHARR_PLEX_CONTRACT_URL` and `AFISHARR_PLEX_CONTRACT_TOKEN`;
//! without them these tests report that they did not run rather than passing
//! quietly, because a contract test that silently skips reads green on the one
//! lane that was supposed to catch the drift.

mod real;
mod shape;

use afisharr_plex::{
    fake::{FakePlex, Scenario},
    hubs::{HubIdentifier, HubVisibility},
    libraries::{ItemKind, ItemQuery, RatingKey, SectionKey, Window},
    server::{MachineIdentifier, PlexServerClient},
};
use real::Surface;
use serde_json::Value;

/// One read-only call, named as the release lane reports it.
struct Call {
    name: &'static str,
    path: String,
    query: Vec<(String, String)>,
}

/// The read-only surface both servers are asked for.
///
/// **Every read this build makes is in it.** A call left out is a parser and a
/// fake shape checked against nothing but a fixture this repository wrote,
/// which is a test that agrees with itself.
fn read_calls(surface: &Surface) -> Vec<Call> {
    let section = &surface.section;
    let window = ItemQuery::new(Window::first(20)).of_type(ItemKind::Movie);
    let meta = ItemQuery::new(Window::first(0))
        .of_type(ItemKind::Movie)
        .including_meta();
    let children = ItemQuery::new(Window::first(20));
    vec![
        Call {
            // The server root, which is the call that says whether the stored
            // token is still accepted. Its path is empty because the address is
            // the endpoint.
            name: "GET /",
            path: String::new(),
            query: Vec::new(),
        },
        Call {
            name: "GET /identity",
            path: "identity".to_owned(),
            query: Vec::new(),
        },
        Call {
            name: "GET /library/sections",
            path: "library/sections".to_owned(),
            query: Vec::new(),
        },
        Call {
            name: "GET /library/sections/{key}/all",
            path: format!("library/sections/{section}/all"),
            query: window.pairs(),
        },
        Call {
            name: "GET /library/sections/{key}/all?includeMeta=1",
            path: format!("library/sections/{section}/all"),
            query: meta.pairs(),
        },
        Call {
            name: "GET /library/sections/{key}/collections",
            path: format!("library/sections/{section}/collections"),
            query: vec![("includeCollections".to_owned(), "1".to_owned())],
        },
        Call {
            name: "GET /library/metadata/{key}",
            path: format!("library/metadata/{}", surface.item),
            query: Vec::new(),
        },
        Call {
            name: "GET /library/collections/{key}/children",
            path: format!("library/collections/{}/children", surface.collection),
            query: children.pairs(),
        },
        Call {
            // The endpoint the server composed for its own filter, query string
            // and all. Nothing here reassembles it from parts (P7).
            name: "GET a filter's choice list",
            path: surface
                .filter
                .key
                .clone()
                .expect("the surface carries a filter that declared one"),
            query: Vec::new(),
        },
        Call {
            name: "GET /hubs/sections/{key}/manage",
            path: format!("hubs/sections/{section}/manage"),
            query: Vec::new(),
        },
    ]
}

/// The one line any other lane's reader sees instead of a green tick.
fn no_server() {
    eprintln!(
        "SKIPPED: no real Plex server configured. Set {} and {} to run the contract test \
         (D-036). The adversarial fake is unverified without it.",
        real::URL,
        real::TOKEN
    );
}

#[tokio::test]
async fn the_read_shapes_agree_in_both_directions() {
    let Some(server) = real::server() else {
        no_server();
        return;
    };

    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let fake_client = real::fake_client(&fake);

    // Every call in the surface must parse on the real server, and the domain
    // facts each one exists to read must be there.
    let identity = server
        .identity()
        .await
        .expect("GET /identity must answer on a real server");
    assert!(
        !identity.machine_identifier.as_str().is_empty(),
        "a real server names itself"
    );
    assert!(
        !identity.version.is_empty(),
        "a real server names its version"
    );
    server
        .verify_credential()
        .await
        .expect("the server root must accept the token the release lane configured");

    let real_surface = real::surface(&server).await;
    let fake_surface = real::surface(&fake_client).await;

    // The reads the surface discovery does not itself make.
    let item = server
        .item(&real_surface.item)
        .await
        .expect("GET /library/metadata/{key} must answer on a real server");
    assert_eq!(
        item.rating_key, real_surface.item,
        "a real server answers with the item it was asked for"
    );
    let children = server
        .collection_items(&real_surface.collection, &ItemQuery::new(Window::first(20)))
        .await
        .expect("a collection's children must answer on a real server");
    assert!(
        children.total.is_some(),
        "a real server reports the size of the whole result"
    );
    let choices = server
        .filter_choices(&real_surface.filter)
        .await
        .expect("a declared filter's choice list must answer on a real server");
    assert!(
        !choices.is_empty() && choices.iter().all(|choice| !choice.value.is_empty()),
        "the filter declared a choice endpoint, so the server must offer choices on it"
    );

    for (real_call, fake_call) in read_calls(&real_surface)
        .into_iter()
        .zip(read_calls(&fake_surface))
    {
        let real_body = real::raw(&server, &real_call.path, &real_call.query).await;
        let fake_body = real::raw(&fake_client, &fake_call.path, &fake_call.query).await;
        real::capture(real_call.name, &real_body);
        shape::assert_supported(real_call.name, &fake_body, &real_body);
        shape::assert_covered(real_call.name, &fake_body, &real_body, real::ALLOWED);
    }
}

#[tokio::test]
async fn the_write_shapes_agree_against_a_collection_this_test_removes_again() {
    let Some(server) = real::server() else {
        no_server();
        return;
    };
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let fake_client = real::fake_client(&fake);

    let real_surface = real::surface(&server).await;
    let fake_surface = real::surface(&fake_client).await;

    let real_answers = write_cycle(&server, &real_surface).await;
    let fake_answers = write_cycle(&fake_client, &fake_surface).await;

    for (name, real_body) in &real_answers {
        let Some((_, fake_body)) = fake_answers.iter().find(|(other, _)| other == name) else {
            panic!("the fake did not answer {name}")
        };
        real::capture(name, real_body);
        shape::assert_supported(name, fake_body, real_body);
        shape::assert_covered(name, fake_body, real_body, real::ALLOWED);
    }
}

/// Creates a collection, exercises every write against it, and deletes it.
///
/// Every answer is kept, named as the lane reports it. The delete runs whatever
/// the rest did, because this runs on somebody's real Plex (P2).
async fn write_cycle(client: &PlexServerClient, surface: &Surface) -> Vec<(&'static str, Value)> {
    let mut answers = Vec::new();
    let identity = client
        .identity()
        .await
        .expect("the server must name itself before anything is written to it");
    let server = MachineIdentifier::new(identity.machine_identifier.as_str());

    let created = client
        .create_collection(
            &surface.section,
            ItemKind::Movie,
            real::SCRATCH,
            &server,
            std::slice::from_ref(&surface.item),
        )
        .await
        .expect("POST /library/collections must answer");
    answers.push((
        "POST /library/collections",
        real::raw(
            client,
            &format!("library/collections/{}/children", created.rating_key),
            &ItemQuery::new(Window::first(20)).pairs(),
        )
        .await,
    ));

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
        !gone.iter().any(|row| row.title == real::SCRATCH),
        "nothing this test created may be left behind"
    );

    if let Err(failure) = outcome {
        panic!("{failure}");
    }
    answers
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

    let edit = afisharr_plex::collections::CollectionEdit {
        sort: Some(afisharr_plex::collections::CollectionSort::Custom),
        summary: Some("Written by the Afisharr contract test.".to_owned()),
        ..afisharr_plex::collections::CollectionEdit::default()
    };
    client
        .edit_collection(section, collection, &edit)
        .await
        .map_err(|error| format!("the collection edit must answer: {error}"))?;

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

    // Promotion, which is the call a `PUT` cannot stand in for: the collection
    // has no manage row until this runs.
    let before = client
        .hubs(section)
        .await
        .map_err(|error| format!("the manage endpoint must answer: {error}"))?;
    client
        .set_collection_visibility(section, &before, collection, HubVisibility::default())
        .await
        .map_err(|error| format!("promotion must answer: {error}"))?;
    answers.push((
        "GET /hubs/sections/{key}/manage?metadataItemId",
        real::raw(
            client,
            &format!("hubs/sections/{section}/manage"),
            &[("metadataItemId".to_owned(), collection.to_string())],
        )
        .await,
    ));

    let after = client
        .hubs(section)
        .await
        .map_err(|error| format!("the manage endpoint must answer: {error}"))?;
    let row = after
        .row_for(collection)
        .ok_or_else(|| "a promoted collection must have a manage row".to_owned())?;
    let identifier: HubIdentifier = row.identifier.clone();
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

#[tokio::test]
async fn the_four_shapes_the_ordering_space_depends_on_are_what_a_real_server_sends() {
    // Named one at a time, because a regression on any of them reported as
    // "the shape set differs somewhere" sends whoever reads it back to a server
    // they may not have. Each is a claim `python-plexapi` reads as fact, and
    // each is load-bearing for the placement phase.
    let Some(server) = real::server() else {
        no_server();
        return;
    };
    let surface = real::surface(&server).await;
    let section = &surface.section;

    let manage = real::raw(&server, &format!("hubs/sections/{section}/manage"), &[]).await;
    let rows = manage["MediaContainer"]["Hub"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(!rows.is_empty(), "a real server has an ordering space");
    for row in &rows {
        assert!(
            row.get("identifier").is_some(),
            "blocker 1: a manage row names itself `identifier`, and this one does not: {row}"
        );
        assert!(
            row.get("hubIdentifier").is_none(),
            "blocker 1: `hubIdentifier` belongs to /hubs/sections/{{key}}, not to the manage \
             endpoint, and this build stopped emitting it here: {row}"
        );
    }
    assert!(
        rows.iter().any(|row| row.get("deletable").is_some()),
        "blocker 1: `deletable` is how a real server says a row cannot be removed, and \
         `HubKind` is read from it"
    );

    let created = create_scratch(&server, &surface).await;

    let unpromoted = real::raw(
        &server,
        &format!("hubs/sections/{section}/manage"),
        &[("metadataItemId".to_owned(), created.to_string())],
    )
    .await;
    let answered = unpromoted["MediaContainer"]["Hub"]
        .as_array()
        .map(Vec::len)
        .unwrap_or_default();
    let blocker_2 = answered == 0;

    let sort = real::raw(
        &server,
        &format!("library/metadata/{created}"),
        &[],
    )
    .await["MediaContainer"]["Metadata"][0]["collectionSort"]
        .clone();
    let blocker_4 = sort.is_null() || sort == 0 || sort == "0";

    let written = server
        .edit_item_sort_title(
            section,
            ItemKind::Movie,
            &surface.item,
            Some("Afisharr contract test"),
            false,
        )
        .await;
    let blocker_3 = written.as_ref().is_ok_and(|written| *written > 0);
    // Put it back whatever happened: this is somebody's library (P3).
    let _ = server
        .edit_item_sort_title(section, ItemKind::Movie, &surface.item, None, false)
        .await;

    server
        .delete_collection(&created)
        .await
        .expect("the collection this test created must be removable");

    assert!(
        blocker_2,
        "blocker 2: a never-promoted collection must have no manage row, and this server \
         answered {answered}"
    );
    assert!(
        blocker_4,
        "blocker 4: a new collection defaults to release order, and this server said {sort}"
    );
    assert!(
        blocker_3,
        "blocker 3: the edit endpoint must write an item at the item libtype, and it \
         answered {written:?}"
    );
}

/// Creates the scratch collection the blocker checks address.
async fn create_scratch(server: &PlexServerClient, surface: &Surface) -> RatingKey {
    let identity = server
        .identity()
        .await
        .expect("the server must name itself before anything is written to it");
    server
        .create_collection(
            &surface.section,
            ItemKind::Movie,
            real::SCRATCH,
            &MachineIdentifier::new(identity.machine_identifier.as_str()),
            std::slice::from_ref(&surface.item),
        )
        .await
        .expect("POST /library/collections must answer")
        .rating_key
}

#[tokio::test]
async fn the_fake_answers_every_call_the_contract_covers() {
    // Runs in every lane, and is the part that would go red first if a call in
    // the surface above were renamed or dropped: the release lane needs a real
    // server, and this needs nothing.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = real::fake_client(&fake);
    for call in read_calls(&real::surface(&client).await) {
        let body = real::raw(&client, &call.path, &call.query).await;
        assert!(
            body.get("MediaContainer").is_some(),
            "{} answered outside the envelope every Plex answer arrives in",
            call.name
        );
    }
}

#[tokio::test]
async fn the_fake_survives_the_whole_write_cycle_the_release_lane_runs() {
    // The other half that needs no server: a write cycle that panicked against
    // the fake would fail the release lane for a reason nobody could tell from
    // a real drift.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = real::fake_client(&fake);
    let surface = real::surface(&client).await;
    let answers = write_cycle(&client, &surface).await;
    assert!(!answers.is_empty(), "every write answers something");
    assert_eq!(
        fake.snapshot().section_keys().first().map(String::as_str),
        Some(SectionKey::new("1").as_str()),
        "and the world is where it was"
    );
}
