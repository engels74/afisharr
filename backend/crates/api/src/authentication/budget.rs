// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Which request budget a call is counted against.
//!
//! One question, asked in two places that cannot both answer it. The router's
//! layer runs before any extractor, so it knows what a request *presents* and
//! never whether the instance accepts it; the extractor knows the second and
//! runs too late for the first. Splitting the answer between them is what keeps
//! anonymous traffic and an accepted credential on separate allowances — and
//! that separation is the whole point, because `trustProxy` is empty by
//! default, so behind the reverse proxy nearly every deployment runs, every
//! caller resolves to the proxy's one address. Counted together, one
//! unauthenticated flood spends what the operator's own interface needs and
//! holds the whole surface at 429 for the rest of the window (PRD §21.4.3).

use std::net::IpAddr;

use axum::http::HeaderMap;
use axum_extra::extract::CookieJar;

use crate::{
    authentication::{Authenticated, Credential},
    error::AppError,
    ratelimit::{Bucket, Decision},
    security::SESSION_COOKIE,
    state::ApiState,
};

impl Credential {
    /// What this credential is counted under in the API budget.
    ///
    /// A server-side identifier either way — a session digest or an API key's
    /// row id — so the budget is one caller's and a caller cannot invent a
    /// second one for themselves.
    #[must_use]
    pub fn budget_key(&self) -> &str {
        match self {
            Credential::Session { digest } => digest,
            Credential::ApiKey { id, .. } => id,
        }
    }
}

/// Whether a request carries something the guard will judge as a credential.
///
/// Presenting one and holding one are different facts, and this answers only
/// the first: it reads headers, never the database. The rate-limit layer needs
/// exactly that much.
///
/// It reads the `Authorization` header through [`guard::bearer_key`] rather than
/// asking whether the header exists, because the two questions have different
/// answers and the gap between them was uncounted traffic. `Authorization: Basic
/// x` and an empty bearer value are headers the guard cannot use: it refuses
/// them before it reaches the arm that counts a refused credential. Reading them
/// here as presented credentials made the layer skip them too, so a caller
/// looping `Authorization: Basic x` against `/api/stream` was metered by
/// nothing, and the limiter reported zero for traffic that was in fact
/// unbounded.
#[must_use]
pub fn presents_credential(headers: &HeaderMap) -> bool {
    super::guard::bearer_key(headers).is_some()
        || CookieJar::from_headers(headers)
            .get(SESSION_COOKIE)
            .is_some()
}

/// Counts one accepted call against that credential's own budget.
///
/// # Errors
/// Returns the rate-limit refusal once the credential's allowance is spent.
pub fn spend_api(state: &ApiState, caller: &Authenticated) -> Result<(), AppError> {
    match state
        .limiter()
        .record(&Bucket::api(caller.credential.budget_key()), None)
    {
        Decision::Allowed => Ok(()),
        Decision::Refused {
            retry_after_seconds,
        } => Err(crate::ratelimit::too_many_requests(retry_after_seconds)),
    }
}

/// Counts one call against its address's anonymous budget.
///
/// Returns the refusal to send in place of the caller's own once that budget is
/// spent, and `None` while it is not — somebody who mistyped a key is told what
/// is wrong, and somebody hammering the instance with invented ones is told to
/// wait.
#[must_use]
pub fn spend_anonymous(state: &ApiState, address: Option<IpAddr>) -> Option<AppError> {
    match state.limiter().record(&Bucket::Anonymous, address) {
        Decision::Allowed => None,
        Decision::Refused {
            retry_after_seconds,
        } => Some(crate::ratelimit::too_many_requests(retry_after_seconds)),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{
        HeaderValue,
        header::{AUTHORIZATION, COOKIE},
    };

    use super::*;

    #[test]
    fn a_bearer_header_and_a_session_cookie_both_read_as_presented_credentials() {
        // The rate-limit layer branches on this before any extractor runs, so a
        // credential it failed to see would be counted as anonymous traffic and
        // spend the budget the sign-in page needs.
        let mut with_key = HeaderMap::new();
        with_key.insert(AUTHORIZATION, HeaderValue::from_static("Bearer abc123"));
        assert!(presents_credential(&with_key));

        let mut with_cookie = HeaderMap::new();
        with_cookie.insert(
            COOKIE,
            HeaderValue::from_str(&format!("{SESSION_COOKIE}=value")).expect("valid"),
        );
        assert!(presents_credential(&with_cookie));

        assert!(!presents_credential(&HeaderMap::new()));
    }

    #[test]
    fn a_header_the_guard_cannot_read_is_not_a_presented_credential() {
        // The gap that left traffic uncounted: the layer skipped these for
        // carrying a header, and the guard refused them before the arm that
        // counts a refused credential, so no bucket ever saw them.
        for header in ["Basic abc123", "Bearer   ", "Bearer", ""] {
            let mut headers = HeaderMap::new();
            headers.insert(AUTHORIZATION, HeaderValue::from_str(header).expect("valid"));
            assert!(
                !presents_credential(&headers),
                "{header:?} is not a credential this instance judges"
            );
        }
    }

    #[test]
    fn an_unrelated_cookie_is_not_a_presented_credential() {
        // Otherwise the CSRF cookie alone would move a signed-out visitor onto
        // the authenticated budget, where nothing they do is ever counted.
        let mut jar = HeaderMap::new();
        jar.insert(COOKIE, HeaderValue::from_static("afisharr_csrf=value"));
        assert!(!presents_credential(&jar));
    }

    #[test]
    fn a_credential_is_counted_under_a_name_the_caller_did_not_choose() {
        // The budget key is the digest this instance stored or the row id it
        // minted, never the bytes the caller sent — otherwise every invented
        // key would open a counter of its own.
        let session = Credential::Session {
            digest: "d".to_owned(),
        };
        let key = Credential::ApiKey {
            id: "K".to_owned(),
            scopes: afisharr_core::api_keys::ScopeSet::NONE,
        };
        assert_eq!(session.budget_key(), "d");
        assert_eq!(key.budget_key(), "K");
    }
}
