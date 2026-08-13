// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Building the cookies this surface sets, with the flags PRD §21.4.2 fixes.

use axum_extra::extract::cookie::{Cookie, SameSite};
use cookie::time::Duration;

use crate::proxy::Scheme;

/// The session cookie.
pub const SESSION_COOKIE: &str = "afisharr_session";

/// The cookie that binds a Plex sign-in attempt to the browser that started it.
///
/// A pin identifier is a public string: it carries no credential, it is handed
/// to the operator to read out, and the account that started the exchange knows
/// it. Completing the exchange, however, mints a session — so without something
/// that only the starting browser holds, anybody who has seen an identifier can
/// have a *different* browser complete it and be handed the resulting cookie.
/// This is that something, and it is what makes the completing request one the
/// CSRF layer judges rather than one it waves through as uncredentialled.
pub const PLEX_PIN_COOKIE: &str = "afisharr_plex_pin";

/// The path the Plex sign-in cookie is scoped to.
///
/// Narrow on purpose: it authorises exactly one exchange, and no other route
/// has any use for it.
pub const PLEX_PIN_COOKIE_PATH: &str = "/api/auth/plex/pin";

/// The CSRF token cookie.
///
/// Readable by script, unlike the other two: the double-submit check needs the
/// SPA to echo it in a header, and a value the page cannot read is a value it
/// cannot echo. Reading it grants nothing — it authenticates nothing on its
/// own, and same-origin script could mint a request anyway.
pub const CSRF_COOKIE: &str = "afisharr_csrf";

/// Builds a cookie with the flags this surface always sets.
///
/// `Secure` follows the resolved scheme rather than being unconditional: an
/// operator reaching a fresh instance over plaintext on their LAN would
/// otherwise be handed a cookie the browser refuses to send back, and would see
/// a login that succeeds and then immediately does not.
///
/// The scheme reaching here is not only the forwarded one. `trustProxy` is
/// empty by default, so a stock deployment behind TLS discards its proxy's
/// `X-Forwarded-Proto` and would resolve as plaintext — setting this cookie
/// without `Secure` on a connection carrying TLS, with nothing saying so. The
/// router resolves a request arriving at the operator's configured `https`
/// `publicOrigin` as HTTPS for exactly that reason, so an operator has a way to
/// state the fact that does not require them to also name their proxy.
#[must_use]
pub fn set<'c>(
    name: &'static str,
    value: String,
    path: &'static str,
    max_age_seconds: i64,
    scheme: Scheme,
    http_only: bool,
) -> Cookie<'c> {
    let mut cookie = Cookie::new(name, value);
    cookie.set_path(path);
    cookie.set_http_only(http_only);
    cookie.set_secure(scheme.is_secure());
    // `Lax` rather than `Strict`: the Plex OAuth variant returns the operator
    // by top-level navigation, and `Strict` withholds the cookie on exactly
    // that request. CSRF protection is always on regardless (PRD §19.6.1).
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(Some(Duration::seconds(max_age_seconds)));
    cookie
}

/// Builds the cookie that removes `name`.
#[must_use]
pub fn expire<'c>(name: &'static str, path: &'static str, scheme: Scheme) -> Cookie<'c> {
    let mut cookie = set(name, String::new(), path, 0, scheme, true);
    cookie.make_removal();
    cookie.set_path(path);
    cookie
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_cookie_is_http_only_and_lax() {
        let cookie = set(
            SESSION_COOKIE,
            "v".to_owned(),
            "/",
            600,
            Scheme::Https,
            true,
        );
        assert_eq!(cookie.http_only(), Some(true));
        assert_eq!(cookie.same_site(), Some(SameSite::Lax));
        assert_eq!(cookie.path(), Some("/"));
    }

    #[test]
    fn secure_follows_the_resolved_scheme() {
        let over_tls = set(
            SESSION_COOKIE,
            "v".to_owned(),
            "/",
            600,
            Scheme::Https,
            true,
        );
        let plaintext = set(SESSION_COOKIE, "v".to_owned(), "/", 600, Scheme::Http, true);
        assert_eq!(over_tls.secure(), Some(true));
        assert_eq!(plaintext.secure(), Some(false));
    }

    #[test]
    fn the_csrf_cookie_is_readable_by_script() {
        let cookie = set(CSRF_COOKIE, "v".to_owned(), "/", 600, Scheme::Https, false);
        assert_eq!(cookie.http_only(), Some(false));
    }

    #[test]
    fn expiring_a_cookie_keeps_the_path_it_was_set_on() {
        // A removal on the wrong path leaves the original cookie in place, and
        // the setup cookie is set on /api/setup rather than on /.
        let cookie = expire("afisharr_setup_claim", "/api/setup", Scheme::Https);
        assert_eq!(cookie.path(), Some("/api/setup"));
        assert_eq!(cookie.value(), "");
    }
}
