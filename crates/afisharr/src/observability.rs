// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the instance says about itself while it runs.

mod components;
mod logging;

pub use components::{Component, components};
pub use logging::{LogGuard, init};
