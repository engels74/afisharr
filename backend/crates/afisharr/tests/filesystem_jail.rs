// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `I-SEC-3` — no path escapes its asset root, over HTTP.
//!
//! The containment rule itself is unit-tested in `afisharr_core::filesystem`
//! against the traversal corpus. This drives the same corpus through the
//! browse route, because a rule that is correct and a route that forgets to
//! call it are indistinguishable from the outside.

mod harness;

use harness::{RunningInstance, TempInstance, Wizard};
use reqwest::{Client, StatusCode};

const PASSWORD: &str = "correct horse battery staple";

fn browser() -> Client {
    Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("the test client must build")
}

/// A signed-in instance with one enabled asset root over a scratch tree.
///
/// The tree holds a file inside the root and a file outside it, which is the
/// only shape the traversal corpus needs.
async fn instance_with_a_root() -> (TempInstance, tempfile::TempDir, RunningInstance, Client) {
    let media = tempfile::TempDir::new().expect("a scratch directory");
    let root = media.path().join("assets");
    std::fs::create_dir_all(root.join("posters")).expect("the root must be creatable");
    std::fs::write(root.join("posters").join("a.png"), b"x").expect("a file inside the root");
    std::fs::write(media.path().join("outside.txt"), b"x").expect("a file outside it");

    let instance = TempInstance::new();

    // The root is inserted before the server starts, because the enabled roots
    // are read once at boot — the same way the running binary reads them.
    {
        let booted = instance.boot().await;
        sqlx::query(
            "INSERT INTO asset_roots (id, path, purpose, is_enabled, created_at)
             VALUES (?1, ?2, 'Browse', 1, 0)",
        )
        .bind("01JROOT0000000000000000000")
        .bind(root.to_string_lossy().to_string())
        .execute(booted.database.readers())
        .await
        .expect_err("the read pool must refuse a write");
        booted.database.close().await;
    }
    insert_root(&instance, &root).await;

    let running = RunningInstance::start(&instance).await;
    let _wizard = Wizard::set_up(&running, "operator", PASSWORD).await;

    let client = browser();
    client
        .post(format!("{}/api/auth/login", running.base_url))
        .json(&serde_json::json!({ "username": "operator", "password": PASSWORD }))
        .send()
        .await
        .expect("the login route must answer");

    (instance, media, running, client)
}

/// Writes the root row through the one write path there is.
async fn insert_root(instance: &TempInstance, root: &std::path::Path) {
    use afisharr_core::storage::WriteOperation;

    struct InsertRoot(String);

    impl WriteOperation for InsertRoot {
        type Output = ();

        async fn execute(self, conn: &mut sqlx::SqliteConnection) -> Result<(), sqlx::Error> {
            sqlx::query(
                "INSERT INTO asset_roots (id, path, purpose, is_enabled, created_at)
                 VALUES ('01JROOT0000000000000000000', ?1, 'Browse', 1, 0)",
            )
            .bind(self.0)
            .execute(&mut *conn)
            .await?;
            Ok(())
        }
    }

    let booted = instance.boot().await;
    booted
        .database
        .writer()
        .submit(InsertRoot(root.to_string_lossy().into_owned()))
        .await
        .expect("the root must be insertable");
    booted.database.close().await;
}

#[tokio::test]
async fn the_root_is_browsable_and_reports_relative_paths() {
    let (_instance, _media, running, client) = instance_with_a_root().await;

    let roots: serde_json::Value = client
        .get(format!("{}/api/files/roots", running.base_url))
        .send()
        .await
        .expect("the roots route must answer")
        .json()
        .await
        .expect("a JSON body");
    let label = roots[0]["label"].as_str().expect("one root").to_owned();

    let listing: serde_json::Value = client
        .get(format!("{}/api/files", running.base_url))
        .query(&[("root", label.as_str()), ("path", "posters")])
        .send()
        .await
        .expect("the browse route must answer")
        .json()
        .await
        .expect("a JSON body");

    assert_eq!(listing["entries"][0]["name"], "a.png");
    assert_eq!(listing["entries"][0]["path"], "posters/a.png");

    running.stop().await;
}

#[tokio::test]
async fn every_traversal_in_the_corpus_is_refused_with_the_root_named() {
    let (_instance, media, running, client) = instance_with_a_root().await;

    let roots: serde_json::Value = client
        .get(format!("{}/api/files/roots", running.base_url))
        .send()
        .await
        .expect("the roots route must answer")
        .json()
        .await
        .expect("a JSON body");
    let label = roots[0]["label"].as_str().expect("one root").to_owned();

    let outside = media
        .path()
        .join("outside.txt")
        .to_string_lossy()
        .into_owned();
    let corpus = [
        "..",
        "../",
        "../outside.txt",
        "posters/../../outside.txt",
        "./../../outside.txt",
        outside.as_str(),
        "/etc",
        "/etc/passwd",
    ];

    for path in corpus {
        let response = client
            .get(format!("{}/api/files", running.base_url))
            .query(&[("root", label.as_str()), ("path", path)])
            .send()
            .await
            .unwrap_or_else(|error| panic!("{path} must answer: {error}"));

        assert!(
            response.status().is_client_error(),
            "{path} answered {}",
            response.status()
        );
        let body: serde_json::Value = response.json().await.expect("a JSON body");
        let message = body["message"].as_str().unwrap_or_default();
        assert!(
            message.contains(&label),
            "{path} was refused without naming the root: {message}"
        );
        assert!(
            !message.contains("outside.txt") && !message.contains("passwd"),
            "{path} disclosed the resolved path: {message}"
        );
    }

    running.stop().await;
}

#[tokio::test]
async fn a_symlink_out_of_the_root_is_refused() {
    let (_instance, media, running, client) = instance_with_a_root().await;

    let link = media.path().join("assets").join("escape");
    #[cfg(unix)]
    std::os::unix::fs::symlink(media.path().join("outside.txt"), &link)
        .expect("the link must be creatable");
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(media.path().join("outside.txt"), &link)
        .expect("the link must be creatable");

    let roots: serde_json::Value = client
        .get(format!("{}/api/files/roots", running.base_url))
        .send()
        .await
        .expect("the roots route must answer")
        .json()
        .await
        .expect("a JSON body");
    let label = roots[0]["label"].as_str().expect("one root").to_owned();

    let response = client
        .get(format!("{}/api/files", running.base_url))
        .query(&[("root", label.as_str()), ("path", "escape")])
        .send()
        .await
        .expect("the browse route must answer");
    assert!(response.status().is_client_error());

    running.stop().await;
}

#[tokio::test]
async fn browsing_requires_a_credential() {
    let (_instance, _media, running, _client) = instance_with_a_root().await;

    let anonymous = reqwest::Client::new();
    let response = anonymous
        .get(format!("{}/api/files/roots", running.base_url))
        .send()
        .await
        .expect("the roots route must answer");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    running.stop().await;
}
