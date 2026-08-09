// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Leases: what stops two logical passes, not two writes.
//!
//! Serialising writes through one actor does not stop a scheduled collection
//! sync and a manual "sync now" interleaving, each writing valid rows that
//! together mean nothing. A lease does (PRD §19.4). Acquisition is one
//! conditional insert-or-update that steals only an expired lease, and a long
//! pass heartbeats: a pass whose lease has gone must abort rather than finish,
//! because another holder may already have started.

mod error;
mod guard;
mod name;
mod store;

pub use error::LeaseError;
pub use guard::LeaseGuard;
pub use name::{LeaseName, LeaseOwner};
pub use store::{Acquire, ClearOwnedBy, Heartbeat, Release, held_by};
