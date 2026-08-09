// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The clock every pass takes as an argument.

use std::fmt;

use crate::time::Timestamp;

/// Supplies the current instant to code that must not read the wall clock itself.
///
/// Domain logic — lease expiry, lifecycle phase, scheduling — is a pure function
/// of its inputs plus the time it is evaluated at. Passing the clock in keeps
/// that function testable at a chosen instant instead of at whatever instant the
/// test happened to run.
pub trait Clock: fmt::Debug + Send + Sync + 'static {
    /// The current instant.
    fn now(&self) -> Timestamp;
}
