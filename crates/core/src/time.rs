// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Time as Afisharr stores and reads it.
//!
//! Instants are integer milliseconds since the Unix epoch (PRD §19.1), and
//! domain logic never reads the wall clock directly — it takes a [`Clock`].

mod clock;
mod fixed_clock;
mod instant;
mod system_clock;

pub use clock::Clock;
pub use fixed_clock::FixedClock;
pub use instant::Timestamp;
pub use system_clock::SystemClock;
