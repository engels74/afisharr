// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # afisharr
//!
//! The binary's wiring: where the instance keeps its files, what it logs, the
//! sequence it boots through, and the commands it exposes.
//!
//! It is a library as well as a binary so the boot sequence can be exercised
//! from `tests/` against a real database, which is the only way to check the
//! ordering guarantees that make a bad upgrade recoverable.

pub mod bootstrap;
pub mod cli;
pub mod configuration;
pub mod interface;
pub mod observability;
pub mod server;
pub mod startup;
