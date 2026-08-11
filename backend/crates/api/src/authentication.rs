// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Who is calling, and how they proved it.
//!
//! Two credentials reach this surface: a session cookie held by a browser, and
//! an API key held by a script. Both resolve to the same [`Authenticated`]
//! value, so no route downstream branches on which one arrived — the one place
//! that difference matters is CSRF, and that decision is made at the perimeter
//! (`crate::security`).
//!
//! First run is the other half of this module. Nothing is reachable until an
//! administrator exists, and no administrator is creatable without the setup
//! claim (`I-SEC-8`, D-045).

pub(crate) mod account_routes;
mod budget;
mod guard;
pub(crate) mod password_login;
mod plex_pin_authorize;
pub(crate) mod plex_pin_poll;
pub(crate) mod plex_pin_start;
pub(crate) mod session;

pub use account_routes::{
    PasswordChange, PasswordChanged, SessionView, change_password, list_sessions, revoke_session,
};
pub use budget::presents_credential;
pub use guard::{Administrator, Authenticated, Credential};
pub(crate) use password_login::ABSENT_ACCOUNT_HASH;
pub use password_login::{Credentials, SignedIn, log_in, log_out, whoami};
pub use plex_pin_poll::{PinState, poll_plex_pin};
pub use plex_pin_start::{PinStarted, StartPin, start_plex_pin};
pub use session::{IssuedSession, issue, revoke};
