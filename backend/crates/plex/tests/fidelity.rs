// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The fake's fidelity contract (D-036), one test per row.
//!
//! Each row is a behaviour an invariant from Phase 4 onward is written against,
//! and each is driven through the real client rather than asserted on the
//! fake's internals: a fake whose misbehaviour only its own state can see is a
//! fake no test of the client can use.

use std::time::Duration;

use afisharr_plex::{
    artwork::ArtworkKind,
    collections::MoveTarget,
    fake::{FakeOperation, FakePlex, Injection, Scenario},
    hubs::{HubIdentifier, HubMove},
    identity::ClientIdentity,
    libraries::{ItemKind, ItemQuery, RatingKey, ScanState, SectionKey, Window},
    server::{
        BindingVerdict, MachineIdentifier, PlexServerClient, ServerAddress, ServerToken,
        verify_binding,
    },
};
use afisharr_sources::outbound::OutboundClient;

fn client_for(fake: &FakePlex) -> PlexServerClient {
    PlexServerClient::new(
        OutboundClient::new("afisharr/test").expect("the transport must build"),
        ClientIdentity::new("01JTESTCLIENT", "Test Instance", "0.1.0").expect("a valid identity"),
        ServerAddress::parse(fake.base_url()).expect("a valid address"),
        ServerToken::new("test-plex-token").expect("a header-safe token"),
    )
}

fn movies() -> SectionKey {
    SectionKey::new("1")
}

fn everything() -> ItemQuery {
    ItemQuery::new(Window::first(500)).of_type(ItemKind::Movie)
}

#[tokio::test]
async fn the_fake_answers_every_call_the_client_makes() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);

    let identity = client.identity().await.expect("the fake answers");
    assert_eq!(identity.machine_identifier.as_str(), "fake-machine-0000");

    let sections = client.sections().await.expect("the fake answers");
    assert_eq!(sections.len(), 2);

    let page = client
        .items(&movies(), &everything())
        .await
        .expect("the fake answers");
    assert_eq!(page.items.len(), 12);

    let item = client
        .item(&page.items[0].rating_key)
        .await
        .expect("the fake answers");
    assert!(item.media().is_some_and(|media| !media.is_empty()));

    let collections = client
        .collections(&movies())
        .await
        .expect("the fake answers");
    assert_eq!(collections.len(), 1);

    let hubs = client.hubs(&movies()).await.expect("the fake answers");
    assert_eq!(hubs.hubs.len(), 2);

    let vocabulary = client
        .vocabulary(&movies(), ItemKind::Movie)
        .await
        .expect("the fake answers");
    let genre = vocabulary.types[0]
        .filters
        .iter()
        .find(|filter| filter.filter == "genre")
        .expect("the fake declares a genre filter")
        .clone();
    let choices = client
        .filter_choices(&genre)
        .await
        .expect("the fake answers");
    assert_eq!(choices.len(), 3);
}

#[tokio::test]
async fn a_move_past_the_precision_budget_reports_success_and_does_not_happen() {
    // Row one of the fidelity contract, and the reason every applied plan is
    // verified by reading the order back (§15.3, `I-CONV-*`).
    let fake = FakePlex::start(Scenario::behaving(1).with_move_budget(1)).await;
    let client = client_for(&fake);

    let hubs = client.hubs(&movies()).await.expect("the fake answers");
    let first = hubs.hubs[0].identifier.clone();
    let second = hubs.hubs[1].identifier.clone();

    client
        .move_hub(&movies(), &second, &HubMove::ToFront)
        .await
        .expect("the first move is inside the budget");
    assert_eq!(
        fake.snapshot().hub_order("1")[0],
        second.as_str(),
        "the first move happened"
    );

    // The second move answers exactly as the first did, and changes nothing.
    client
        .move_hub(&movies(), &first, &HubMove::ToFront)
        .await
        .expect("the call still answers 200 past the budget");
    assert_eq!(
        fake.snapshot().hub_order("1")[0],
        second.as_str(),
        "past the budget the order is unchanged and the call still succeeded"
    );
}

#[tokio::test]
async fn a_collection_item_move_past_the_budget_is_silent_in_the_same_way() {
    let fake = FakePlex::start(Scenario::behaving(1).with_move_budget(0)).await;
    let client = client_for(&fake);

    let before = fake.snapshot().collection_items("15001");
    client
        .move_collection_item(
            &RatingKey::new("15001"),
            &RatingKey::new(before[2].clone()),
            &MoveTarget::ToFront,
        )
        .await
        .expect("the call answers 200");
    assert_eq!(fake.snapshot().collection_items("15001"), before);
}

#[tokio::test]
async fn artwork_arrives_in_more_than_one_format_this_build_cannot_read() {
    // Row two (`I-ID-2`, `I-RENDER-2`): the pass must not abort, and the raw
    // value must survive so the doctor page can report what was seen.
    let fake = FakePlex::start(
        Scenario::behaving(5)
            .holding(120, 0)
            .unrecognised_artwork(2),
    )
    .await;

    let page = client_for(&fake)
        .items(
            &movies(),
            &ItemQuery::new(Window::first(500)).of_type(ItemKind::Movie),
        )
        .await
        .expect("an unreadable artwork format does not fail the fetch");

    let kinds: Vec<ArtworkKind> = page
        .items
        .iter()
        .filter_map(|item| item.thumb.as_ref())
        .map(afisharr_plex::artwork::ArtworkRef::kind)
        .collect();
    assert!(kinds.contains(&ArtworkKind::ServerPath), "{kinds:?}");
    assert!(kinds.contains(&ArtworkKind::InternalScheme), "{kinds:?}");
    assert!(kinds.contains(&ArtworkKind::Unrecognised), "{kinds:?}");
}

#[tokio::test]
async fn the_same_item_comes_back_under_a_new_key_on_a_later_fetch() {
    // Row three (`I-ID-1`, `I-ID-3`, `I-SRC-6`): the key changed, the guid did
    // not, and a client that treated the key as identity now has two rows.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);

    let before = client
        .items(&movies(), &everything())
        .await
        .expect("the fake answers");
    fake.churn_rating_keys();
    let after = client
        .items(&movies(), &everything())
        .await
        .expect("the fake answers");

    assert_ne!(before.items[0].rating_key, after.items[0].rating_key);
    assert_eq!(before.items[0].guid, after.items[0].guid);
}

#[tokio::test]
async fn churn_can_be_scheduled_to_land_between_two_windows_of_one_pass() {
    let fake = FakePlex::start(Scenario::behaving(1).holding(20, 0)).await;
    let client = client_for(&fake);
    fake.churn_after_fetches(1);

    let first = client
        .items(&movies(), &ItemQuery::new(Window { start: 0, size: 10 }))
        .await
        .expect("the fake answers");
    let second = client
        .items(
            &movies(),
            &ItemQuery::new(Window {
                start: 10,
                size: 10,
            }),
        )
        .await
        .expect("the fake answers");

    assert!(
        first.items[0].rating_key.as_str().len() < second.items[0].rating_key.as_str().len(),
        "the second window is served from re-keyed items"
    );
}

#[tokio::test]
async fn an_item_still_being_indexed_reports_nothing_rather_than_nothing_there() {
    // Row four (`I-EVID-*`): the difference between "this film has no file"
    // and "Plex has not looked yet".
    let fake = FakePlex::start(Scenario::behaving(11).holding(60, 0).partially_scanned(3)).await;

    let page = client_for(&fake)
        .items(&movies(), &everything())
        .await
        .expect("the fake answers");

    let indexing: Vec<_> = page
        .items
        .iter()
        .filter(|item| item.scan == ScanState::Indexing)
        .collect();
    assert!(!indexing.is_empty(), "the scenario asked for partial scans");
    for item in indexing {
        assert_eq!(item.media(), None, "{}", item.rating_key);
        assert!(item.media_as_reported().is_empty());
    }
    assert!(
        page.items
            .iter()
            .any(|item| item.scan == ScanState::Complete),
        "a library where nothing is indexed is a different scenario"
    );
}

#[tokio::test]
async fn sort_titles_vary_in_value_presence_and_lock_independently() {
    // Row five (`I-REV-3`): all three round-trip, and the state a restore gets
    // wrong is an absent value with a locked field.
    let fake = FakePlex::start(
        Scenario::behaving(23)
            .holding(120, 0)
            .absent_sort_titles(3)
            .locked_sort_titles(3),
    )
    .await;

    let page = client_for(&fake)
        .items(&movies(), &everything())
        .await
        .expect("the fake answers");

    assert!(page.items.iter().any(|item| item.sort_title.is_present()));
    assert!(page.items.iter().any(|item| !item.sort_title.is_present()));
    assert!(page.items.iter().any(|item| item.sort_title.is_locked()));
    assert!(page.items.iter().any(|item| !item.sort_title.is_locked()));
    assert!(
        page.items
            .iter()
            .any(|item| !item.sort_title.is_present() && item.sort_title.is_locked()),
        "absent and locked at once is the state a restore gets wrong"
    );
}

#[tokio::test]
async fn a_chosen_operation_refuses_mid_pass_while_the_rest_keep_working() {
    // Row six (`I-EVID-1`, `I-ACQ-*`): the failure lands on the third window,
    // not the first, so what a pass does with work already done is testable.
    let fake = FakePlex::start(Scenario::behaving(1).holding(30, 0).failing(
        FakeOperation::Items,
        2,
        Injection::Refuse { status: 503 },
    ))
    .await;
    let client = client_for(&fake);

    for window in 0..2_u32 {
        client
            .items(
                &movies(),
                &ItemQuery::new(Window {
                    start: window * 10,
                    size: 10,
                }),
            )
            .await
            .unwrap_or_else(|error| panic!("window {window} must answer: {error}"));
    }

    let error = client
        .items(
            &movies(),
            &ItemQuery::new(Window {
                start: 20,
                size: 10,
            }),
        )
        .await
        .expect_err("the third window is the one the scenario refuses");
    assert_eq!(error.refused_status(), Some(503));
    assert!(error.server_answered(), "a refusal is an answer");

    // Another operation is untouched: the injection is per-call, not a server
    // that has fallen over.
    client
        .sections()
        .await
        .expect("the section list still answers");
}

#[tokio::test]
async fn a_stalled_operation_is_reported_as_no_answer_rather_than_as_a_refusal() {
    // The other half of row six. A stall is the failure a retry policy waiting
    // for an exception waits forever on, so it must be told apart from a 5xx.
    let fake = FakePlex::start(Scenario::behaving(1).failing(
        FakeOperation::Sections,
        0,
        Injection::Stall,
    ))
    .await;
    let client = PlexServerClient::new(
        OutboundClient::with_deadline(
            "afisharr/test",
            afisharr_sources::outbound::Deadline::of(Duration::from_millis(300)),
        )
        .expect("the transport must build"),
        ClientIdentity::new("01JTESTCLIENT", "Test Instance", "0.1.0").expect("a valid identity"),
        ServerAddress::parse(fake.base_url()).expect("a valid address"),
        ServerToken::new("test-plex-token").expect("a header-safe token"),
    );

    let error = client
        .sections()
        .await
        .expect_err("a stalled call never answers");
    assert!(
        !error.server_answered(),
        "a stall is not an answer, and an adapter that read it as one would \
         reconcile a library to empty: {error}"
    );
}

#[tokio::test]
async fn a_changed_machine_identifier_is_detected_and_never_reconciled() {
    // Row seven, and `I-ID-5`: the operator pointed the same address at another
    // machine. Every rating key in the database now means something else, and
    // nothing may be rebound without an explicit decision.
    let fake = FakePlex::start(Scenario::behaving(1).identified_as("server-a")).await;
    let client = client_for(&fake);

    let first = client.identity().await.expect("the fake answers");
    let recorded = first.machine_identifier.clone();
    assert_eq!(
        verify_binding(Some(&recorded), &first.machine_identifier),
        BindingVerdict::Bound {
            identifier: MachineIdentifier::new("server-a")
        }
    );

    fake.becomes_a_different_server("server-b");

    let second = client.identity().await.expect("the fake answers");
    let verdict = verify_binding(Some(&recorded), &second.machine_identifier);
    assert!(verdict.blocks(), "{verdict:?}");
    assert_eq!(
        verdict,
        BindingVerdict::DifferentServer {
            expected: MachineIdentifier::new("server-a"),
            found: MachineIdentifier::new("server-b"),
        },
        "both sides are named, because the operator's decision needs both"
    );
}

#[tokio::test]
async fn two_runs_from_one_seed_are_byte_identical() {
    // The property that makes every row above usable: a scenario is
    // reproducible from its seed alone, so a failure is a bug rather than a
    // flake somebody eventually mutes.
    async fn transcript(seed: u64) -> String {
        let fake = FakePlex::start(
            Scenario::behaving(seed)
                .holding(40, 6)
                .unrecognised_artwork(3)
                .partially_scanned(4)
                .absent_sort_titles(5)
                .locked_sort_titles(6),
        )
        .await;
        let client = client_for(&fake);
        let mut lines = Vec::new();
        for section in ["1", "2"] {
            let page = client
                .items(
                    &SectionKey::new(section),
                    &ItemQuery::new(Window::first(500)),
                )
                .await
                .expect("the fake answers");
            for item in page.items {
                lines.push(format!(
                    "{}|{}|{:?}|{:?}|{}|{:?}",
                    item.rating_key,
                    item.guid.unwrap_or_default(),
                    item.sort_title.value(),
                    item.sort_title.is_locked(),
                    item.scan == ScanState::Complete,
                    item.thumb.map(|thumb| thumb.as_str().to_owned()),
                ));
            }
        }
        lines.join("\n")
    }

    let first = transcript(2024).await;
    let second = transcript(2024).await;
    assert_eq!(first, second, "the same seed must replay byte for byte");
    assert_ne!(
        first,
        transcript(2025).await,
        "a different seed must be a different world, or the seed means nothing"
    );
}

#[tokio::test]
async fn labels_written_through_the_client_are_read_back_by_it() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);
    let key = RatingKey::new("10001");

    client
        .edit_labels(
            &movies(),
            ItemKind::Movie,
            &key,
            &afisharr_plex::labels::LabelEdit::adding("afisharr"),
        )
        .await
        .expect("the fake answers");
    assert_eq!(
        fake.snapshot().labels("10001"),
        Some(vec!["afisharr".to_owned()])
    );

    client
        .edit_labels(
            &movies(),
            ItemKind::Movie,
            &key,
            &afisharr_plex::labels::LabelEdit::removing("afisharr"),
        )
        .await
        .expect("the fake answers");
    assert_eq!(fake.snapshot().labels("10001"), Some(Vec::new()));
}

#[tokio::test]
async fn a_collection_created_through_the_client_holds_what_it_was_given() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);
    let server = MachineIdentifier::new(fake.machine_identifier());

    let created = client
        .create_collection(
            &movies(),
            ItemKind::Movie,
            "Made By A Test",
            &server,
            &[RatingKey::new("10001"), RatingKey::new("10002")],
        )
        .await
        .expect("the fake answers");

    assert_eq!(
        fake.snapshot()
            .collection_items(created.rating_key.as_str()),
        ["10001", "10002"]
    );

    let page = client
        .collection_items(&created.rating_key, &ItemQuery::new(Window::first(50)))
        .await
        .expect("the fake answers");
    assert_eq!(page.items.len(), 2);
}

#[tokio::test]
async fn a_sort_title_written_through_the_client_carries_its_lock_state() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);

    client
        .edit_collection(
            &movies(),
            &RatingKey::new("15001"),
            &afisharr_plex::collections::CollectionEdit {
                sort_title: Some(("!001 Promoted".to_owned(), true)),
                ..afisharr_plex::collections::CollectionEdit::default()
            },
        )
        .await
        .expect("the fake answers");
    assert_eq!(
        fake.snapshot().collection_sort_title("15001"),
        Some((Some("!001 Promoted".to_owned()), true))
    );

    // And unlocking is a write the fake honours, because a restore that left
    // the field locked would otherwise look correct (`I-REV-3`).
    client
        .edit_collection(
            &movies(),
            &RatingKey::new("15001"),
            &afisharr_plex::collections::CollectionEdit {
                sort_title: Some(("Promoted".to_owned(), false)),
                ..afisharr_plex::collections::CollectionEdit::default()
            },
        )
        .await
        .expect("the fake answers");
    assert_eq!(
        fake.snapshot().collection_sort_title("15001"),
        Some((Some("Promoted".to_owned()), false))
    );
}

#[tokio::test]
async fn a_poster_uploaded_through_the_client_reaches_the_item() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);

    client
        .upload_poster(
            &RatingKey::new("10001"),
            afisharr_plex::artwork::ArtworkUpload::png(vec![0x89, b'P', b'N', b'G']),
        )
        .await
        .expect("the fake answers");

    let item = client
        .item(&RatingKey::new("10001"))
        .await
        .expect("the fake answers");
    assert!(
        item.thumb
            .expect("the item has a poster")
            .as_str()
            .ends_with("upload-4"),
        "the uploaded bytes reached the item"
    );
}

#[tokio::test]
async fn the_hub_visibility_axes_are_written_one_at_a_time() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);

    client
        .set_hub_visibility(
            &movies(),
            &HubIdentifier::new("collection.15001"),
            afisharr_plex::hubs::HubVisibility {
                own_home: false,
                shared_home: true,
                recommended: false,
            },
        )
        .await
        .expect("the fake answers");

    let hubs = client.hubs(&movies()).await.expect("the fake answers");
    let hub = hubs
        .hubs
        .iter()
        .find(|hub| hub.identifier == HubIdentifier::new("collection.15001"))
        .expect("the collection hub is there");
    assert!(!hub.visibility.own_home);
    assert!(hub.visibility.shared_home);
    assert!(!hub.visibility.recommended);
}
