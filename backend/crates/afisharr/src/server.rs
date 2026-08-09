// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Turning a booted instance into a listening HTTP server.
//!
//! Everything the router needs is assembled here, in the one place that knows
//! about every crate: the wiring layer. Each feature's state is built by the
//! feature and handed over whole (§24.6.3).

mod listener;
mod wiring;

pub use listener::{Serving, serve};
pub use wiring::build_state;
