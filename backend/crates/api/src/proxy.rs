// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the reverse proxy in front of Afisharr is allowed to tell it.
//!
//! `trustProxy` is a list of addresses and CIDR ranges, never a boolean
//! (D-029, PRD §21.4.3). A boolean is the trap: turn it on while the instance
//! is also reachable directly, and an attacker sets `X-Forwarded-For` per
//! request. Every per-IP limit then becomes decorative while continuing to
//! report that it is working perfectly — against an identifier the attacker
//! chooses.
//!
//! One module answers both questions that depend on it: which address a
//! request is really from (`I-SEC-1`), and whether it really arrived over
//! HTTPS (which decides `Strict-Transport-Security` and the `Secure` cookie
//! flag).

mod configured_origin;
mod edge;
mod forwarded;
mod origin;
mod peer;
mod trusted;

pub(crate) use forwarded::Claim;
pub use origin::{PublicOrigin, PublicOriginError};
pub use peer::{ClientContext, Scheme};
pub use trusted::{TrustedProxies, TrustedProxyError, canonical};
