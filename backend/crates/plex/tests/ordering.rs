// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The ordering space's rows of the fidelity contract (D-036).
//!
//! Split out of `fidelity.rs` because these four share one subject: what the
//! placement algorithm is allowed to assume about moving and hiding rows. The
//! two §15.3 rows say a move past the precision budget reports success and does
//! not happen; the two §15.1/§15.5 rows say the three visibility axes are
//! independent and that one of Plex's own rows is an anchor. Each is driven
//! through the real client, for the reason every row is: a misbehaviour only
//! the fake's own state can see is one no test of the client can use.

use afisharr_plex::{
    collections::MoveTarget,
    fake::{FakePlex, Scenario},
    hubs::{HubIdentifier, HubKind, HubMove, HubVisibility},
    identity::ClientIdentity,
    libraries::{RatingKey, SectionKey},
    server::{PlexServerClient, ServerAddress, ServerToken},
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
async fn the_hub_visibility_axes_are_written_one_at_a_time() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);

    client
        .set_hub_visibility(
            &movies(),
            &HubIdentifier::new("collection.15001"),
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
        .find(|hub| hub.identifier == HubIdentifier::new("collection.15001"))
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
        "and it is still one of Plex's own rows"
    );
}
