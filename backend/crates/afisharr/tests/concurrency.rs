// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One writer, and one holder per lease.

// Integration tests may unwrap: a failed setup step is a failed test, and the
// panic names the line. The rule is about non-test paths (§24.2.3).
#![allow(clippy::unwrap_used)]
mod harness;

use std::sync::Arc;

use afisharr_core::{
    leases::{ClearOwnedBy, LeaseError, LeaseGuard, LeaseName, LeaseOwner, held_by},
    time::{Clock, FixedClock, Timestamp},
};
use harness::TempInstance;

const TTL: i64 = 30_000;

fn clock() -> Arc<FixedClock> {
    Arc::new(FixedClock::at(Timestamp::from_millis(1_700_000_000_000)))
}

/// `I-DATA-2`/`I-DATA-3` — two logical passes racing for one lease never both proceed.
#[tokio::test]
async fn only_one_of_two_racing_passes_takes_the_lease() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;
    let clock = clock();
    let name = LeaseName::CollectionPass {
        definition_id: "DEF1".to_owned(),
    };

    let scheduled = LeaseGuard::acquire(
        booted.database.writer(),
        booted.database.readers(),
        clock.clone(),
        name.clone(),
        LeaseOwner::task("INSTANCE", "scheduler"),
        TTL,
    )
    .await
    .expect("the first pass takes the lease");

    let manual = LeaseGuard::acquire(
        booted.database.writer(),
        booted.database.readers(),
        clock.clone(),
        name.clone(),
        LeaseOwner::task("INSTANCE", "sync-now"),
        TTL,
    )
    .await;

    match manual {
        Err(LeaseError::Held { holder, .. }) => {
            assert!(
                holder.contains("scheduler"),
                "the refusal must name the holder: {holder}"
            );
        }
        other => panic!("a second pass must be refused, got {other:?}"),
    }

    scheduled
        .heartbeat()
        .await
        .expect("the holder still holds it");
    scheduled.release().await.expect("releasing");
    assert!(
        held_by(booted.database.readers(), &name)
            .await
            .expect("reading")
            .is_none()
    );
    booted.database.close().await;
}

#[tokio::test]
async fn an_expired_lease_is_stolen_and_the_previous_holder_learns_it_lost() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;
    let clock = clock();
    let name = LeaseName::PlacementPass {
        library_id: "LIB1".to_owned(),
    };

    let crashed = LeaseGuard::acquire(
        booted.database.writer(),
        booted.database.readers(),
        clock.clone(),
        name.clone(),
        LeaseOwner::task("INSTANCE", "placement"),
        TTL,
    )
    .await
    .expect("the first pass takes the lease");

    clock.advance(TTL + 1);

    let successor = LeaseGuard::acquire(
        booted.database.writer(),
        booted.database.readers(),
        clock.clone(),
        name.clone(),
        LeaseOwner::task("OTHER", "placement"),
        TTL,
    )
    .await
    .expect("an expired lease is stealable");

    // The crashed pass must abort rather than complete: the successor may
    // already have started, and two passes finishing is what the lease prevents.
    let lost = crashed
        .heartbeat()
        .await
        .expect_err("the previous holder must learn it lost");
    assert!(matches!(lost, LeaseError::Lost { .. }), "{lost:?}");

    successor.release().await.expect("releasing");
    booted.database.close().await;
}

#[tokio::test]
async fn leases_over_different_scopes_do_not_serialise_each_other() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;
    let clock = clock();

    let placement = LeaseGuard::acquire(
        booted.database.writer(),
        booted.database.readers(),
        clock.clone(),
        LeaseName::PlacementPass {
            library_id: "LIB1".to_owned(),
        },
        LeaseOwner::task("INSTANCE", "placement"),
        TTL,
    )
    .await
    .expect("placement takes its lease");

    let lifecycle = LeaseGuard::acquire(
        booted.database.writer(),
        booted.database.readers(),
        clock.clone(),
        LeaseName::LifecyclePass {
            library_id: "LIB1".to_owned(),
        },
        LeaseOwner::task("INSTANCE", "lifecycle"),
        TTL,
    )
    .await
    .expect("lifecycle over the same library is a different scope");

    placement.release().await.expect("releasing");
    lifecycle.release().await.expect("releasing");
    booted.database.close().await;
}

/// Startup clears this instance's leases and leaves another process's alone.
#[tokio::test]
async fn startup_clears_only_the_leases_this_instance_owns() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;
    let clock = clock();

    let ours = LeaseName::Job {
        job_id: "overlay-sweep".to_owned(),
    };
    let theirs = LeaseName::Job {
        job_id: "another-process".to_owned(),
    };
    let claim = LeaseName::SetupClaim;

    for (name, owner) in [
        (
            ours.clone(),
            LeaseOwner::task(&booted.instance.instance_id, "overlay-sweep"),
        ),
        (
            theirs.clone(),
            LeaseOwner::task("SOME-OTHER-INSTANCE", "overlay-sweep"),
        ),
        // The setup claim's owner is a cookie digest, so it must never match
        // the instance prefix: an interrupted setup survives a restart (D-046).
        (claim.clone(), LeaseOwner::token("f00dcafe")),
    ] {
        LeaseGuard::acquire(
            booted.database.writer(),
            booted.database.readers(),
            clock.clone(),
            name,
            owner,
            TTL,
        )
        .await
        .expect("taking the lease");
    }

    let cleared = booted
        .database
        .writer()
        .submit(ClearOwnedBy {
            instance_id: booted.instance.instance_id.clone(),
        })
        .await
        .expect("clearing our own leases");

    assert_eq!(cleared, 1);
    let readers = booted.database.readers();
    assert!(held_by(readers, &ours).await.expect("reading").is_none());
    assert!(held_by(readers, &theirs).await.expect("reading").is_some());
    assert!(held_by(readers, &claim).await.expect("reading").is_some());
    booted.database.close().await;
}

/// D-024 — the write actor is the only path that can mutate.
#[tokio::test]
async fn the_read_pool_cannot_write() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;

    let refusal = sqlx::query("DELETE FROM principals")
        .execute(booted.database.readers())
        .await
        .expect_err("the read pool must refuse a mutation");

    assert!(refusal.to_string().contains("readonly"), "{refusal}");

    let survivors: i64 = sqlx::query_scalar("SELECT count(*) FROM principals")
        .fetch_one(booted.database.readers())
        .await
        .expect("counting principals");
    assert_eq!(survivors, 3);
    booted.database.close().await;
}

/// The clock is injected, so lease expiry is testable without waiting.
#[tokio::test]
async fn a_lease_expires_by_the_injected_clock_rather_than_by_elapsed_time() {
    let instance = TempInstance::new();
    let booted = instance.boot().await;
    let clock = clock();
    let name = LeaseName::LifecyclePass {
        library_id: "LIB1".to_owned(),
    };

    let guard = LeaseGuard::acquire(
        booted.database.writer(),
        booted.database.readers(),
        clock.clone(),
        name.clone(),
        LeaseOwner::task("INSTANCE", "lifecycle"),
        TTL,
    )
    .await
    .expect("taking the lease");

    let holder = held_by(booted.database.readers(), &name)
        .await
        .expect("reading")
        .expect("held");
    assert_eq!(holder.expires_at, clock.now().plus_millis(TTL));

    guard.release().await.expect("releasing");
    booted.database.close().await;
}
