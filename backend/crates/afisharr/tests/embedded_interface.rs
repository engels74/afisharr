// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Task 1.7: the binary serves the SPA out of itself.
//!
//! Skipped when the frontend has not been built, because a checkout that has
//! not run `bun run build` has nothing to embed and failing over that would
//! make `cargo nextest run` depend on a second toolchain.
//!
//! CI builds the SPA and sets `AFISHARR_REQUIRE_SPA`, which turns the skip into
//! a failure. A lane that forgot to build the frontend would otherwise report
//! green while never exercising the one-file claim at all.

mod harness;

use afisharr::interface::EmbeddedInterface;
use harness::{RunningInstance, TempInstance};
use reqwest::{Client, StatusCode};

/// Whether to skip, and a refusal to skip where the SPA was required.
fn skip_without_spa() -> bool {
    if EmbeddedInterface::is_present() {
        return false;
    }
    assert!(
        std::env::var_os("AFISHARR_REQUIRE_SPA").is_none(),
        "AFISHARR_REQUIRE_SPA is set but no SPA is embedded: the lane built the \
         binary before the interface, so this assertion never ran"
    );
    eprintln!("no SPA in this build; run `bun run build` in frontend/ to exercise this");
    true
}

fn client() -> Client {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("the test client must build")
}

#[tokio::test]
async fn a_page_route_is_answered_by_the_embedded_shell() {
    if skip_without_spa() {
        return;
    }

    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let client = client();

    // A deep link on a full page load: the shell answers, and the client
    // router reads the URL and renders the right route.
    // Every one of the six primary destinations plus Settings and its
    // sub-pages, plus the first-run journey: Task 1.11 asks that each resolves
    // to a routed page rather than a 404.
    for path in [
        "/",
        "/dashboard",
        "/collections",
        "/design",
        "/home-screen",
        "/lifecycle",
        "/doctor",
        "/settings",
        "/settings/plex",
        "/settings/integrations",
        "/settings/libraries",
        "/settings/users",
        "/settings/general",
        "/settings/teardown",
        "/settings/about",
        "/setup",
        "/setup/admin",
        "/login",
    ] {
        let response = client
            .get(format!("{}{path}", running.base_url))
            .send()
            .await
            .unwrap_or_else(|error| panic!("{path} must answer: {error}"));

        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("text/html; charset=utf-8"),
            "{path}"
        );
        // The shell names which bundles to load, so it must never be cached.
        assert_eq!(
            response
                .headers()
                .get("cache-control")
                .and_then(|value| value.to_str().ok()),
            Some("no-cache"),
            "{path}"
        );
        assert!(
            response.text().await.unwrap_or_default().contains("<html"),
            "{path} did not answer with the shell"
        );
    }

    running.stop().await;
}

#[tokio::test]
async fn the_policy_admits_the_shell_the_binary_actually_serves() {
    // The whole point of hashing at boot: `'unsafe-inline'` never appears in
    // `script-src`, and the one inline bootstrap the SPA carries is admitted by
    // the digest of its own bytes. A change to the SPA changes the hash with it.
    if skip_without_spa() {
        return;
    }

    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;

    let response = client()
        .get(format!("{}/dashboard", running.base_url))
        .send()
        .await
        .expect("the shell must answer");
    let policy = response
        .headers()
        .get("content-security-policy")
        .and_then(|value| value.to_str().ok())
        .expect("every response carries a policy")
        .to_owned();
    let html = response.text().await.expect("a body");

    let script_src = policy
        .split("; ")
        .find(|directive| directive.starts_with("script-src"))
        .expect("the policy names script-src");
    assert!(
        !script_src.contains("unsafe-inline"),
        "the policy admitted inline script wholesale: {script_src}"
    );

    let inline = afisharr_api::interface::inline_script_digests(&html);
    assert!(
        !inline.is_empty(),
        "the shell should carry one inline bootstrap; the hashing has nothing to prove otherwise"
    );
    for digest in inline {
        assert!(
            script_src.contains(&digest),
            "the policy does not admit the script the binary served: {script_src}"
        );
    }

    running.stop().await;
}

#[tokio::test]
async fn a_fingerprinted_bundle_is_cacheable_for_a_year() {
    if skip_without_spa() {
        return;
    }

    let instance = TempInstance::new();
    let running = RunningInstance::start(&instance).await;
    let client = client();

    // Find a bundle the shell names, and ask for it the way a browser would.
    let html = client
        .get(format!("{}/dashboard", running.base_url))
        .send()
        .await
        .expect("the shell must answer")
        .text()
        .await
        .expect("a body");
    let bundle = html
        .split('"')
        // Vite writes lower-case extensions; matching the path it emits is
        // the point, so this is deliberately not a case-folded comparison.
        .find(|token| {
            token.starts_with("/_app/immutable/")
                && std::path::Path::new(token)
                    .extension()
                    .is_some_and(|ext| ext == "js")
        })
        .expect("the shell names at least one bundle")
        .to_owned();

    let response = client
        .get(format!("{}{bundle}", running.base_url))
        .send()
        .await
        .expect("the bundle must answer");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("public, max-age=31536000, immutable")
    );

    running.stop().await;
}
