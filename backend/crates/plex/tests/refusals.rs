// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a server refuses, and what it declines to say.
//!
//! Three cases the fake could not previously produce at all, and each of them
//! is a fact the product decides on:
//!
//! - A token a server no longer accepts. The revoked-credential state was
//!   provable only by an injected refusal, never by the condition itself, so
//!   nothing checked that the check works.
//! - A key the server does not hold. A real Plex answers `404`; some answer an
//!   empty container; the fake asserted one of the two, so half the clients in
//!   the world went untested.
//! - A file check nobody asked for. `Part.accessible` requires the request to
//!   ask, and the fake sent it always — so the `None` case this build's own
//!   documentation is written against never happened, and a broken-media
//!   overlay could not be shown to be honest (P1).

mod harness;

use afisharr_plex::{
    fake::{FakePlex, Scenario},
    libraries::{ItemQuery, RatingKey, Window},
};
use harness::{ask, client_for, client_with_token, movies};

#[tokio::test]
async fn a_server_that_names_a_token_refuses_every_other_one() {
    let fake = FakePlex::start(Scenario::behaving(1).accepting_token("the-live-one")).await;

    client_with_token(&fake, "the-live-one")
        .verify_credential()
        .await
        .expect("the token the scenario named is accepted");

    let error = client_with_token(&fake, "a-revoked-one")
        .verify_credential()
        .await
        .expect_err("a revoked token is not a working connection");
    assert_eq!(error.refused_status(), Some(401));
    assert!(
        error.server_answered(),
        "a refusal is an answer, and an outage is not"
    );
}

#[tokio::test]
async fn a_request_carrying_no_token_at_all_is_refused() {
    // What a claimed server does. Nothing here decided it: the condition is
    // the absence of a credential, and the answer is the one Plex gives.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let answer = ask(&fake, "library/sections", &[]).await;
    assert_eq!(answer.status, 401);
    assert!(answer.body.contains("Unauthorized"), "{}", answer.body);
}

#[tokio::test]
async fn the_refusal_is_rendered_in_whichever_form_the_request_asked_for() {
    // A client that parses the body of a `401` should read a status and a code
    // rather than a parse error on top of the refusal it was already handling.
    let fake = FakePlex::start(Scenario::behaving(1).accepting_token("the-live-one")).await;
    let xml = ask(&fake, "identity", &[("x-plex-token", "wrong")]).await;
    assert!(xml.body.contains("<Response "), "{}", xml.body);

    let json = ask(
        &fake,
        "identity",
        &[("x-plex-token", "wrong"), ("accept", "application/json")],
    )
    .await;
    let parsed: serde_json::Value = serde_json::from_str(&json.body).expect("json parses");
    assert_eq!(parsed["Response"]["status"], "Unauthorized");
}

#[tokio::test]
async fn a_key_the_server_does_not_hold_is_refused_by_default() {
    // The case a re-keyed item walks into (`I-ID-1`): the caller holds a key
    // that meant something an hour ago.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let error = client_for(&fake)
        .item(&RatingKey::new("99999"))
        .await
        .expect_err("a real server refuses a key it does not hold");
    assert_eq!(error.refused_status(), Some(404));
}

#[tokio::test]
async fn a_scenario_can_choose_the_empty_container_instead() {
    // Both shapes exist on real servers and a client has to survive each. The
    // fake asserting one is what left the other untested.
    let fake = FakePlex::start(Scenario::behaving(1).answering_empty_for_missing_items()).await;
    let answer = harness::ask_as_a_reference_client(&fake, "library/metadata/99999").await;
    assert_eq!(answer.status, 200);
    assert!(answer.body.contains("size=\"0\""), "{}", answer.body);
}

#[tokio::test]
async fn a_part_says_nothing_about_the_file_unless_the_request_asked() {
    // `accessible: None` is Plex not having looked, and it is the ordinary
    // case rather than the exceptional one.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = client_for(&fake);

    let quiet = client
        .items(&movies(), &ItemQuery::new(Window::first(1)))
        .await
        .expect("the fake answers");
    let part = &quiet.items[0]
        .media()
        .expect("the scan is complete")
        .first()
        .expect("the item has media")
        .parts[0];
    assert_eq!(part.accessible, None, "nobody asked Plex to look");
    assert_eq!(part.exists, None);

    let checked = client
        .items(
            &movies(),
            &ItemQuery::new(Window::first(1)).checking_files(),
        )
        .await
        .expect("the fake answers");
    let part = &checked.items[0]
        .media()
        .expect("the scan is complete")
        .first()
        .expect("the item has media")
        .parts[0];
    assert_eq!(
        part.accessible,
        Some(true),
        "the request asked, so Plex said"
    );
    assert_eq!(part.exists, Some(true));
}

#[tokio::test]
async fn a_scenario_can_withhold_the_media_attributes_a_server_reports_only_sometimes() {
    let fake = FakePlex::start(Scenario::behaving(1).withholding_media_details()).await;
    let page = client_for(&fake)
        .items(&movies(), &ItemQuery::new(Window::first(1)))
        .await
        .expect("the fake answers");
    let media = &page.items[0]
        .media()
        .expect("the scan is complete")
        .first()
        .expect("the item has media")
        .clone();
    assert_eq!(media.aspect_ratio, None);
    assert_eq!(media.video_profile, None);
    assert_eq!(media.video_frame_rate, None);
    assert_eq!(
        media.video_resolution.as_deref(),
        Some("1080"),
        "the facts a server always sends are still there"
    );
}

#[tokio::test]
async fn the_attributes_a_server_does_send_are_read_rather_than_invented() {
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let page = client_for(&fake)
        .items(&movies(), &ItemQuery::new(Window::first(1)))
        .await
        .expect("the fake answers");
    let item = &page.items[0];
    let media = item
        .media()
        .expect("the scan is complete")
        .first()
        .expect("the item has media");
    assert_eq!(media.video_profile.as_deref(), Some("high"));
    assert_eq!(media.video_frame_rate.as_deref(), Some("24p"));
    assert!(media.aspect_ratio.is_some());
    assert!(
        !item.external_guids.is_empty(),
        "the external ids a resolver matches on"
    );
    assert_eq!(item.originally_available_at.as_deref(), Some("1980-05-25"));
}
