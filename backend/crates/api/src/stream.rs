// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The one multiplexed SSE connection (PRD §9).
//!
//! One connection per client, established after authentication, carrying every
//! topic. The stream is an accelerator and never a source of truth: every
//! surface it feeds is correct after a plain page load with no stream at all,
//! and a reconnect refetches rather than replaying (`I-UX-9`).
//!
//! There are no producers yet — jobs and source health arrive in later phases.
//! What exists here is the transport, the topic vocabulary, and the heartbeat,
//! so the first producer publishes to a stream that already works and is
//! already tested.

mod event;
mod hub;
pub(crate) mod route;

pub use event::{StreamEvent, Topic};
pub use hub::{HEARTBEAT_SECONDS, StreamHub};
pub use route::stream;
