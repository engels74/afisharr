// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where a hosted plex.tv sign-in is allowed to return the operator.
//!
//! The open redirect these close: the caller posts any `forwardUrl` it likes,
//! the endpoint embeds it in a genuine `app.plex.tv/auth` URL, and whoever
//! completes the sign-in lands on the attacker's page carrying Afisharr's name
//! and Plex's. The only thing standing between those two facts is what the
//! target is compared against — which is why it is compared against the
//! configured origin and against nothing the request carries (`I-SEC-1`).

mod harness;

use harness::{PlexTvStub, RunningInstance, TempInstance, Wizard, browser};
use reqwest::StatusCode;

const PASSWORD: &str = "correct horse battery staple";

/// A configured instance whose Plex client points at `stub`.
async fn configured(instance: &TempInstance, stub: &PlexTvStub) -> RunningInstance {
    let running = RunningInstance::start_against_plex(instance, &stub.base_url).await;
    let _wizard = Wizard::set_up(&running, "operator", PASSWORD).await;
    running
}

/// One start of a hosted sign-in, with `host` on the request if given.
async fn start_oauth(
    running: &RunningInstance,
    forward_url: &str,
    host: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut request = browser()
        .post(format!("{}/api/auth/plex/pin", running.base_url))
        .json(&serde_json::json!({
            "oauth": true,
            "forwardUrl": forward_url,
        }));
    if let Some(host) = host {
        request = request.header(reqwest::header::HOST, host);
    }
    let response = request.send().await.expect("the start route must answer");
    let status = response.status();
    (status, response.json().await.expect("a JSON body"))
}

#[tokio::test]
async fn a_return_to_this_instance_is_allowed() {
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;

    let target = format!("{}/login", running.base_url);
    let (status, body) = start_oauth(&running, &target, None).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    let url = body["authorizationUrl"]
        .as_str()
        .expect("the OAuth variant carries a sign-in URL");
    assert!(url.starts_with("https://app.plex.tv/auth#"), "{url}");

    running.stop().await;
    stub.stop().await;
}

#[tokio::test]
async fn a_return_to_somebody_else_is_refused() {
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;

    let (status, body) = start_oauth(&running, "https://evil.example/steal", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "invalid");
    assert_eq!(body["pointer"], "/forwardUrl");

    // And nothing was spent on it: no attempt reached plex.tv, so nothing is
    // left behind for the refused request either.
    assert_eq!(
        stub.pins_created(),
        0,
        "a refused return target must not create a pin"
    );

    running.stop().await;
    stub.stop().await;
}

#[tokio::test]
async fn a_host_header_naming_another_site_does_not_make_it_this_instance() {
    // The whole of the fix. `Host` is written by whoever is calling: reached
    // directly, or through a proxy that passes on whatever authority it is
    // given, `Host: evil.example` beside `forwardUrl: http://evil.example/...`
    // compared equal to itself and minted a genuine authorization URL.
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;

    let (status, body) = start_oauth(
        &running,
        // The scheme this instance is really reached over, so nothing about
        // the request disagrees with the target: every part of a comparison
        // against the request itself passes.
        "http://evil.example/steal",
        Some("evil.example"),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a caller must not be able to declare which origin is this instance: {body}"
    );
    assert_eq!(body["code"], "invalid");
    assert_eq!(stub.pins_created(), 0);

    running.stop().await;
    stub.stop().await;
}

#[tokio::test]
async fn an_instance_with_no_configured_origin_refuses_and_names_the_setting() {
    // Nothing to compare against is not the same as anything goes. The code
    // sign-in still works — it needs no return address — and the refusal names
    // the setting rather than blaming the address the operator is sitting on.
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = RunningInstance::start_without_public_origin(&instance, &stub.base_url).await;
    let _wizard = Wizard::set_up(&running, "operator", PASSWORD).await;

    let target = format!("{}/login", running.base_url);
    let (status, body) = start_oauth(&running, &target, None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "invalid");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("publicOrigin"),
        "{body}"
    );
    assert_eq!(stub.pins_created(), 0);

    // The code variant is unaffected: it hands nobody an address for this
    // instance, so it has nothing to prove.
    let by_code = browser()
        .post(format!("{}/api/auth/plex/pin", running.base_url))
        .json(&serde_json::json!({ "oauth": false }))
        .send()
        .await
        .expect("the start route must answer");
    assert_eq!(by_code.status(), StatusCode::OK);

    running.stop().await;
    stub.stop().await;
}
