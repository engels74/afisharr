// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a sign-in does to the linked account's row.
//!
//! A plex.tv account is matched on its numeric id and refreshed from what
//! plex.tv now reports (P4). Usernames are globally unique here, so "refreshed"
//! has a boundary: a name another account already holds is a name this row
//! cannot take, and the sign-in that discovers it has already spent its pin.

mod harness;

use afisharr_core::{accounts, storage::WriteOperation};
use harness::{Attempt, PlexTvStub, RunningInstance, TempInstance, Wizard};
use reqwest::StatusCode;

const PASSWORD: &str = "correct horse battery staple";

/// A configured instance whose Plex client points at `stub`.
async fn configured(instance: &TempInstance, stub: &PlexTvStub) -> RunningInstance {
    let running = RunningInstance::start_against_plex(instance, &stub.base_url).await;
    let _wizard = Wizard::set_up(&running, "operator", PASSWORD).await;
    running
}

/// Adds a linked, active, non-administering Plex account called `username`.
async fn add_linked_account(running: &RunningInstance, plex_account_id: i64, username: &str) {
    struct AddLinked(i64, String);

    impl WriteOperation for AddLinked {
        type Output = ();

        async fn execute(self, conn: &mut sqlx::SqliteConnection) -> Result<(), sqlx::Error> {
            sqlx::query(
                "INSERT INTO users
                   (id, kind, username, password_hash, plex_account_id, is_admin,
                    created_at, updated_at)
                 VALUES ('01JVIEWER000000000000000000', 'Plex', ?2, NULL, ?1, 0, 0, 0)",
            )
            .bind(self.0)
            .bind(self.1)
            .execute(&mut *conn)
            .await?;
            Ok(())
        }
    }

    running
        .booted
        .database
        .writer()
        .submit(AddLinked(plex_account_id, username.to_owned()))
        .await
        .expect("the linked account must be writable");
}

/// The username the account bound to `plex_account_id` currently holds.
async fn username_of(running: &RunningInstance, plex_account_id: i64) -> String {
    accounts::find_by_plex_account(running.booted.database.readers(), plex_account_id)
        .await
        .expect("the query must run")
        .expect("the linked account exists")
        .username
}

#[tokio::test]
async fn a_rename_to_a_free_name_is_taken() {
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;
    add_linked_account(&running, 777, "viewer").await;

    stub.account_is(777);
    stub.username_is("viewer-renamed");
    stub.authorize();

    let attempt = Attempt::start(&running, false).await;
    let authorized = attempt.poll_body(&running).await;
    assert_eq!(authorized["state"], "authorized");
    assert_eq!(authorized["username"], "viewer-renamed");
    assert_eq!(username_of(&running, 777).await, "viewer-renamed");

    running.stop().await;
    stub.stop().await;
}

#[tokio::test]
async fn a_rename_onto_another_account_keeps_the_local_name_and_still_signs_in() {
    // The failure this closes: the refresh writes plex.tv's username into a
    // globally unique column, the administrator already holds that name, and
    // the statement fails — as a 500, inside a sign-in whose pin attempt is
    // already claimed. The operator cannot retry it and cannot get in, because
    // what is wrong is a name on somebody else's service.
    let stub = PlexTvStub::start().await;
    let instance = TempInstance::new();
    let running = configured(&instance, &stub).await;
    add_linked_account(&running, 777, "viewer").await;

    stub.account_is(777);
    stub.username_is("operator");
    stub.authorize();

    let attempt = Attempt::start(&running, false).await;
    let response = attempt.poll(&running).await;
    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("a JSON body");
    assert_eq!(
        status,
        StatusCode::OK,
        "a name plex.tv chose must not decide whether an operator can sign in: {body}"
    );
    assert_eq!(body["state"], "authorized");

    // The local name is the row's own, and the account that holds it keeps it.
    assert_eq!(username_of(&running, 777).await, "viewer");
    let administrator = accounts::find_by_username(running.booted.database.readers(), "operator")
        .await
        .expect("the query must run")
        .expect("the administrator exists");
    assert!(
        administrator.plex_account_id.is_none(),
        "the administrator's row must not have been taken over"
    );

    // And the session it issued is real.
    assert_eq!(
        attempt
            .client
            .get(format!("{}/api/auth/session", running.base_url))
            .send()
            .await
            .expect("the session route must answer")
            .status(),
        StatusCode::OK
    );

    running.stop().await;
    stub.stop().await;
}
