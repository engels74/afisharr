// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The adversarial Plex fake (D-036).
//!
//! A stub does what it is told, and the failures worth testing are the ones
//! where Plex does not. Every invariant from Phase 4 onward that concerns
//! identity, evidence, convergence, or reversibility is written against a
//! server that misbehaves in a specific, named way — so the fake reproduces
//! each behaviour in the fidelity contract on demand, and does so
//! deterministically from one seed.
//!
//! | Behaviour | Invariants that need it |
//! | --- | --- |
//! | A move that reports success and does not happen | `I-CONV-*` |
//! | Artwork URLs in unrecognised formats | `I-ID-2`, `I-RENDER-2` |
//! | Rating-key churn | `I-ID-1`, `I-ID-3`, `I-SRC-6` |
//! | Partial scan states | `I-EVID-*` |
//! | Sort titles with independent value, presence, and lock | `I-REV-3` |
//! | Timeouts and 5xx at a chosen operation | `I-EVID-1`, `I-ACQ-1`–`I-ACQ-3` |
//! | A changed machine identifier | `I-ID-5` |
//!
//! **It is not a Plex emulator.** It answers the surface [`crate::server`]
//! calls and is allowed to be wrong about everything else; a release-lane
//! contract test against a real server is what keeps it truthful.
//!
//! **Determinism is what makes it useful.** Every misbehaviour is triggered by
//! an explicit scenario and seeded from one value, because a fake that
//! misbehaves randomly produces flaky tests, which get muted, which is worse
//! than not having one.

mod collection_routes;
mod hub_routes;
mod instance;
mod item_routes;
mod json;
mod library;
mod plan;
mod routes;
mod scenario;
mod seed;
mod server;
mod state;
mod vocabulary;

pub use plan::{FakeOperation, Injection};
pub use scenario::Scenario;
pub use seed::Seed;
pub use server::{FakePlex, WorldSnapshot};
pub use state::{FakeCollection, FakeHub, FakeItem, FakeLibrary};
