// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the wiring layer hands the router.
//!
//! This is a wiring layer and nothing else. Each feature on this surface owns
//! its own state type — the limiter owns its buckets, the stream owns its
//! topics, the setup gate owns the token store — and [`ApiState`] holds one
//! handle to each. It grows a field when a feature is added and never a method
//! per feature (§24.6.3).

mod context;
mod instance_identity;

pub use context::{ApiState, ApiStateParts};
pub use instance_identity::InstanceIdentity;
