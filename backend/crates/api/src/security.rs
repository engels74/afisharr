// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The response headers every answer carries, and CSRF, which has no toggle.
//!
//! Both are middleware over the whole router rather than anything a handler
//! opts into. A header applied by a handler is a header that is present on the
//! routes somebody remembered and missing on the one they did not, which is
//! precisely what `I-SEC-2` is written to catch.

mod cookies;
mod csrf;
mod headers;

pub use cookies::{
    CSRF_COOKIE, PLEX_PIN_COOKIE, PLEX_PIN_COOKIE_PATH, SESSION_COOKIE, expire, set,
};
pub use csrf::{CSRF_HEADER, CsrfDecision, judge_csrf};
pub use headers::{ContentSecurityPolicy, apply_security_headers};
