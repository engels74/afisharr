// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The server-side half of a plex.tv PIN login.
//!
//! The PIN and OAuth flows are multi-request: create a pin, present something,
//! poll until a token appears. That needs a row (PRD §19.6). The token it
//! yields is not part of that row — it goes to `secrets`, sealed, and the row
//! records only that the login succeeded.

mod store;

pub use store::{
    CompletePinLogin, PinLogin, PinLoginResult, RecordPinLogin, find as find_pin_login,
};
