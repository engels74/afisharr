// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The ordering space's rows of the fidelity contract (D-036).
//!
//! Split out of `fidelity.rs` because these share one subject: what the
//! placement algorithm is allowed to assume about promoting, moving, and
//! hiding rows. The §15.3 rows say a move past the precision budget reports
//! success and does not happen; the §15.1 and §15.5 rows say the three
//! visibility axes are independent, that one of Plex's own rows is an anchor,
//! and that a collection is in the library before it is in the ordering space.
//! Each is driven through the real client, for the reason every row is: a
//! misbehaviour only the fake's own state can see is one no test of the client
//! can use.

use afisharr_plex::{
    collections::MoveTarget,
    fake::{FakePlex, Scenario},
    hubs::{HubIdentifier, HubKind, HubMove, HubVisibility},
    identity::ClientIdentity,
    libraries::{ItemKind, RatingKey, SectionKey},
    server::{MachineIdentifier, PlexServerClient, ServerAddress, ServerToken},
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

/// The collection the default world builds in the movie library.
fn seeded_collection() -> RatingKey {
    RatingKey::new("15001")
}

/// The row that collection is promoted under.
fn seeded_row() -> HubIdentifier {
    HubIdentifier::new("custom.collection.1.15001")
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
            &seeded_collection(),
            &RatingKey::new(before[2].clone()),
            &MoveTarget::ToFront,
        )
        .await
        .expect("the call answers 200");
    assert_eq!(fake.snapshot().collection_items("15001"), before);
}

#[tokio::test]
async fn one_collections_budget_runs_out_while_another_collections_does_not() {
    // One counter across every sequence made this case unreachable, and left an
    // escalation-ladder test unable to say which sequence had run out (§15.3).
    let fake = FakePlex::start(Scenario::behaving(1).with_move_budget(1)).await;
    let client = client_for(&fake);
    let server = MachineIdentifier::new(fake.machine_identifier());

    let second = client
        .create_collection(
            &movies(),
            ItemKind::Movie,
            "A second collection",
            &server,
            &[
                RatingKey::new("10004"),
                RatingKey::new("10005"),
                RatingKey::new("10006"),
            ],
        )
        .await
        .expect("the fake answers");

    // Spend the first collection's whole budget.
    for item in ["10003", "10002"] {
        client
            .move_collection_item(
                &seeded_collection(),
                &RatingKey::new(item),
                &MoveTarget::ToFront,
            )
            .await
            .expect("the call answers 200 either way");
    }
    assert_eq!(
        fake.snapshot().collection_items("15001")[0],
        "10003",
        "the second move was past the budget and did nothing"
    );

    client
        .move_collection_item(
            &second.rating_key,
            &RatingKey::new("10006"),
            &MoveTarget::ToFront,
        )
        .await
        .expect("the fake answers");
    assert_eq!(
        fake.snapshot().collection_items(second.rating_key.as_str())[0],
        "10006",
        "the second collection has a budget of its own"
    );
}

#[tokio::test]
async fn the_hub_visibility_axes_are_written_one_at_a_time() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);

    client
        .set_hub_visibility(
            &movies(),
            &seeded_row(),
            HubVisibility {
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
        .find(|hub| hub.identifier == seeded_row())
        .expect("the collection hub is there");
    assert!(!hub.visibility.own_home);
    assert!(hub.visibility.shared_home);
    assert!(!hub.visibility.recommended);
}

#[tokio::test]
async fn one_of_plexs_own_rows_cannot_be_unpromoted() {
    // The anchor rule (§15.1), and the reason the placement algorithm plans
    // around a native row instead of moving it out of the way: the call
    // answers, and the row stays exactly where it was. A fake that obeyed here
    // would pass a planner reaching for a recovery move Plex does not offer.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);
    let native = HubIdentifier::new("home.continue.1");

    client
        .set_hub_visibility(&movies(), &native, HubVisibility::default())
        .await
        .expect("the fake answers, as Plex does");

    let hubs = client.hubs(&movies()).await.expect("the fake answers");
    let hub = hubs
        .hubs
        .iter()
        .find(|hub| hub.identifier == native)
        .expect("the native row is there");
    assert!(
        hub.visibility.own_home && hub.visibility.shared_home,
        "an anchor keeps both home surfaces whatever it is asked"
    );
    assert_eq!(
        hub.kind,
        HubKind::Native,
        "and it is still one of Plex's own rows, because it says it cannot be removed"
    );
}

#[tokio::test]
async fn every_row_the_manage_endpoint_answers_is_addressable_and_classified() {
    // The four attributes a client reads off a manage row. A row missing any
    // of them is a row the placement algorithm cannot plan with, and until now
    // the fake sent none of the last three.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let listing = client_for(&fake)
        .hubs(&movies())
        .await
        .expect("the fake answers");

    assert_eq!(listing.unidentifiable, 0, "every row names itself");
    assert_eq!(listing.hubs.len(), 2);
    assert_eq!(listing.hubs[0].kind, HubKind::Native);
    assert_eq!(listing.hubs[1].kind, HubKind::Collection);
    assert!(
        listing.hubs[1].names_collection(&seeded_collection()),
        "a collection row names the collection behind it"
    );
}

#[tokio::test]
async fn a_created_collection_is_in_the_library_and_not_yet_in_the_ordering_space() {
    // Two states, not one. The fake used to put every new collection straight
    // into the manage answer, so a promotion path that never promoted anything
    // passed (`plexapi/collection.py:207-215`).
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);
    let server = MachineIdentifier::new(fake.machine_identifier());

    let created = client
        .create_collection(
            &movies(),
            ItemKind::Movie,
            "Made By A Test",
            &server,
            &[RatingKey::new("10001")],
        )
        .await
        .expect("the fake answers");

    let listing = client.hubs(&movies()).await.expect("the fake answers");
    assert!(
        listing.row_for(&created.rating_key).is_none(),
        "nothing has promoted it"
    );
    assert!(!fake.snapshot().is_promoted(created.rating_key.as_str()));

    // And promoting it through the client's own path puts it there.
    client
        .set_collection_visibility(
            &movies(),
            &listing,
            &created.rating_key,
            HubVisibility {
                own_home: true,
                shared_home: false,
                recommended: false,
            },
        )
        .await
        .expect("the fake answers");

    let promoted = client.hubs(&movies()).await.expect("the fake answers");
    let row = promoted
        .row_for(&created.rating_key)
        .expect("the collection is in the space now");
    assert_eq!(row.kind, HubKind::Collection);
    assert!(row.visibility.own_home);
    assert!(!row.visibility.shared_home);
}

#[tokio::test]
async fn writing_the_visibility_of_a_collection_already_in_the_space_does_not_promote_it_twice() {
    // The other half of the choice: a row that is there is written, never
    // added again. One implementation of the rule, so a caller cannot pick the
    // wrong half (P7).
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);

    let listing = client.hubs(&movies()).await.expect("the fake answers");
    client
        .set_collection_visibility(
            &movies(),
            &listing,
            &seeded_collection(),
            HubVisibility {
                own_home: false,
                shared_home: false,
                recommended: true,
            },
        )
        .await
        .expect("the fake answers");

    let after = client.hubs(&movies()).await.expect("the fake answers");
    assert_eq!(after.hubs.len(), 2, "no second row for one collection");
    let row = after
        .row_for(&seeded_collection())
        .expect("the row is still there");
    assert!(!row.visibility.own_home);
    assert!(row.visibility.recommended);
}
