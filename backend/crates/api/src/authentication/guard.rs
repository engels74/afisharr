// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The extractor every protected route takes.

use afisharr_core::{
    api_keys::{self, TouchApiKey},
    digest,
    sessions::{self, TouchSession, Validity},
    time::Timestamp,
};
use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use axum_extra::extract::CookieJar;

use crate::{
    error::{AppError, ErrorCode},
    security::SESSION_COOKIE,
    state::ApiState,
};

/// How long a session's `last_seen_at` may lag before it is written again.
///
/// The idle window is seven days, so second-granularity precision buys nothing
/// and costs one serialised write per request through the single write actor
/// (D-024). A minute of slack keeps the window honest and the hot path clean.
const TOUCH_INTERVAL_MILLIS: i64 = 60 * 1000;

/// Which credential a caller presented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    /// A browser session.
    Session {
        /// The digest the session is stored under.
        digest: String,
    },
    /// An API key.
    ApiKey {
        /// The key's identifier.
        id: String,
    },
}

/// A caller who has proved who they are.
#[derive(Debug, Clone)]
pub struct Authenticated {
    /// The account the caller is acting as.
    pub user_id: String,
    /// Whether that account holds administrator rights.
    pub is_admin: bool,
    /// What they presented.
    pub credential: Credential,
}

impl Authenticated {
    /// The session digest, when the caller is a browser.
    #[must_use]
    pub fn session_digest(&self) -> Option<&str> {
        match &self.credential {
            Credential::Session { digest } => Some(digest),
            Credential::ApiKey { .. } => None,
        }
    }
}

impl FromRequestParts<ApiState> for Authenticated {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ApiState,
    ) -> Result<Self, Self::Rejection> {
        let now = state.clock().now();

        if let Some(presented) = bearer_key(parts) {
            return from_api_key(state, &presented, now).await;
        }

        let jar = CookieJar::from_headers(&parts.headers);
        let Some(cookie) = jar.get(SESSION_COOKIE) else {
            return Err(unauthenticated());
        };
        from_session(state, cookie.value(), now).await
    }
}

/// A caller who has proved who they are **and** holds administrator rights.
///
/// Tier 0 is an admin-only product (D-007): the filesystem browser, the
/// instance's API keys, the Plex connection, and the event stream are one
/// operator's control panel over their own server, and none of them is scoped
/// to the account that asked. `users.is_admin` can still be `0` — a Plex
/// account linked for viewing, a row edited by hand — and such an account holds
/// a session this surface accepts. Without this extractor the whole documented
/// admin-only surface is ordinary authenticated access.
///
/// A second extractor rather than a check inside [`Authenticated`], because
/// "who is calling" and "may they do this" are different questions: signing
/// out, reading one's own session, and changing one's own password are
/// self-scoped and need the first without the second.
#[derive(Debug, Clone)]
pub struct Administrator(
    /// The caller, once their rights are established.
    pub Authenticated,
);

impl FromRequestParts<ApiState> for Administrator {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ApiState,
    ) -> Result<Self, Self::Rejection> {
        let caller = Authenticated::from_request_parts(parts, state).await?;
        if !caller.is_admin {
            return Err(AppError::of(
                ErrorCode::Forbidden,
                "That account does not administer this instance.",
            ));
        }
        Ok(Self(caller))
    }
}

/// The value of an `Authorization: Bearer` header, if there is one.
fn bearer_key(parts: &Parts) -> Option<String> {
    let raw = parts.headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, value) = raw.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

async fn from_api_key(
    state: &ApiState,
    presented: &str,
    now: Timestamp,
) -> Result<Authenticated, AppError> {
    let digest = digest::hex(presented.as_bytes());
    let record = api_keys::find_by_digest(state.database().readers(), &digest)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(unauthenticated)?;

    if !record.is_active() {
        // A revoked key is refused on its next use, and the refusal is the same
        // one an unknown key gets: telling a caller that their key existed once
        // is telling them something.
        return Err(unauthenticated());
    }

    // Last-used is recorded even when the request that follows fails: the
    // question the interface answers is "is this key still in use", and a key
    // making refused calls is very much in use.
    let _ = state
        .database()
        .writer()
        .submit(TouchApiKey {
            id: record.id.clone(),
            at: now,
        })
        .await;

    // An API key acts for the account that created it. A key whose creator was
    // deleted acts for nobody and is refused rather than escalated.
    let user_id = record.created_by.clone().ok_or_else(unauthenticated)?;
    let user = afisharr_core::accounts::find_by_id(state.database().readers(), &user_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(unauthenticated)?;
    if !user.is_active() {
        return Err(unauthenticated());
    }

    Ok(Authenticated {
        user_id,
        is_admin: user.is_admin,
        credential: Credential::ApiKey { id: record.id },
    })
}

async fn from_session(
    state: &ApiState,
    presented: &str,
    now: Timestamp,
) -> Result<Authenticated, AppError> {
    let digest = digest::hex(presented.as_bytes());
    let session = sessions::find_by_digest(state.database().readers(), &digest)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(unauthenticated)?;

    match session.validity(now) {
        Validity::Active => {}
        Validity::Revoked | Validity::Idle | Validity::Expired => return Err(unauthenticated()),
    }

    let user = afisharr_core::accounts::find_by_id(state.database().readers(), &session.user_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(unauthenticated)?;
    if !user.is_active() {
        return Err(unauthenticated());
    }

    if session.last_seen_at.millis_until(now) >= TOUCH_INTERVAL_MILLIS {
        let _ = state
            .database()
            .writer()
            .submit(TouchSession {
                digest: digest.clone(),
                at: now,
            })
            .await;
    }

    Ok(Authenticated {
        user_id: user.id,
        is_admin: user.is_admin,
        credential: Credential::Session { digest },
    })
}

/// One refusal for every way of not being signed in.
///
/// Absent, expired, revoked, unknown, and belonging-to-a-disabled-account all
/// answer identically. Distinguishing them tells a caller which of the five
/// they achieved, and none of the five is something the interface renders
/// differently.
fn unauthenticated() -> AppError {
    AppError::of(ErrorCode::Unauthenticated, "Sign in to continue.")
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderValue, Request};

    use super::*;

    fn parts_with(header: Option<&str>) -> Parts {
        let mut builder = Request::builder().uri("/");
        if let Some(value) = header {
            builder = builder.header(AUTHORIZATION, HeaderValue::from_str(value).expect("valid"));
        }
        builder
            .body(())
            .expect("the request must build")
            .into_parts()
            .0
    }

    #[test]
    fn a_bearer_header_yields_its_key() {
        assert_eq!(
            bearer_key(&parts_with(Some("Bearer abc123"))).as_deref(),
            Some("abc123")
        );
    }

    #[test]
    fn the_scheme_is_matched_case_insensitively() {
        assert_eq!(
            bearer_key(&parts_with(Some("bearer abc123"))).as_deref(),
            Some("abc123")
        );
    }

    #[test]
    fn another_scheme_is_not_read_as_a_key() {
        assert_eq!(bearer_key(&parts_with(Some("Basic abc123"))), None);
    }

    #[test]
    fn an_empty_bearer_value_is_not_a_key() {
        assert_eq!(bearer_key(&parts_with(Some("Bearer   "))), None);
    }

    #[test]
    fn no_header_is_no_key() {
        assert_eq!(bearer_key(&parts_with(None)), None);
    }

    #[test]
    fn a_session_caller_exposes_its_digest_and_a_key_caller_does_not() {
        let session = Authenticated {
            user_id: "U".to_owned(),
            is_admin: true,
            credential: Credential::Session {
                digest: "d".to_owned(),
            },
        };
        let key = Authenticated {
            user_id: "U".to_owned(),
            is_admin: true,
            credential: Credential::ApiKey { id: "K".to_owned() },
        };
        assert_eq!(session.session_digest(), Some("d"));
        assert_eq!(key.session_digest(), None);
    }
}
