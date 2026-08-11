// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Completing a plex.tv sign-in.
//!
//! A `POST`, and not a `GET`, because of what the authorised branch does: it
//! consumes the attempt, stores a token, and sets a session cookie. A `GET` is
//! reachable by cross-site navigation and by a prefetch, and the CSRF layer
//! exempts every safe method — so with a `GET` here, anybody who has seen an
//! attempt identifier (the account that started it, for one) could have another
//! person's browser complete the exchange and be handed the session it minted.
//! The attempt cookie set at the start closes the other half: the request is
//! now credentialled, so the CSRF layer judges it, and the credential is one
//! only the browser that started the exchange holds.

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use afisharr_core::{plex_pin, time::Timestamp};
use afisharr_plex::pin::PinPoll;
use axum::{
    Json,
    extract::{Path, State},
};
use axum_extra::extract::CookieJar;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    authentication::{
        plex_pin_authorize::{PlexIdentity, authorize},
        plex_pin_start::plex_failure,
    },
    error::{AppError, AppResult, ErrorCode, Problem},
    proxy::ClientContext,
    ratelimit::Bucket,
    security::{PLEX_PIN_COOKIE, PLEX_PIN_COOKIE_PATH, expire},
    state::ApiState,
};

/// What one poll found.
///
/// Three states, and the client renders three different things. Folding
/// `expired` into `pending` would leave an operator watching a spinner for a
/// code that will never be accepted (P1).
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum PinState {
    /// plex.tv has not seen the operator finish. Poll again.
    Pending,
    /// A token arrived and a session was created.
    //
    // `rename_all` again, on the variant. The enum-level attribute renames the
    // *variants* of an internally-tagged enum and not their fields, so without
    // this the body carries `user_id` while every other body on this surface
    // carries `userId` — and the generated client reads a field that is not
    // there.
    #[serde(rename_all = "camelCase")]
    Authorized {
        /// The account now signed in.
        user_id: String,
        /// The account name.
        username: String,
        /// Whether that account administers this instance.
        ///
        /// Reported rather than assumed. A linked Plex account that is not an
        /// administrator gets a session with `is_admin = false`, and a client
        /// that recorded it as an administrator would route the operator into
        /// admin-only pages that answer 403 (`I-UX-2`).
        is_admin: bool,
    },
    /// The pin's window closed without a token.
    Expired,
}

/// Polls a Plex sign-in, and signs the operator in when the token arrives.
#[utoipa::path(
    post,
    path = "/api/auth/plex/pin/{id}",
    tag = "authentication",
    params(("id" = String, Path, description = "The attempt returned by the start call")),
    responses(
        (status = 200, description = "The attempt's current state", body = PinState),
        (status = 403, description = "The attempt belongs to another browser, the Plex account is not linked to an Afisharr account, or setup has not been completed on this instance", body = Problem),
        (status = 404, description = "No such attempt", body = Problem),
        (status = 409, description = "The client identifier changed, or the attempt was already completed", body = Problem),
        (status = 429, description = "Too many calls to plex.tv", body = Problem),
        (status = 502, description = "plex.tv could not be reached", body = Problem),
    ),
)]
pub async fn poll_plex_pin(
    State(state): State<ApiState>,
    client: ClientContext,
    jar: CookieJar,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> AppResult<(CookieJar, Json<PinState>)> {
    // Before the attempt is even looked up. The identifier alone proves
    // nothing about who is asking, and this route mints a session.
    if !holds_attempt(&jar, &id) {
        return Err(AppError::of(
            ErrorCode::Forbidden,
            "That sign-in was started somewhere else. Start it again here.",
        ));
    }

    let now = state.clock().now();
    let attempt = plex_pin::find_pin_login(state.database().readers(), &id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::of(ErrorCode::NotFound, "That sign-in attempt is not known."))?;

    if !attempt.is_open(now) {
        close(&state, &attempt.id, plex_pin::PinLoginResult::Expired, now).await;
        return Ok((forget_attempt(jar, client), Json(PinState::Expired)));
    }

    // The identifier the pin was created under has to still be this instance's.
    // A pin issued under another one yields a token plex.tv accepts once and
    // refuses forever after, which is a failure nothing downstream explains.
    if attempt.client_identifier != state.identity().client_identifier {
        close(&state, &attempt.id, plex_pin::PinLoginResult::Aborted, now).await;
        return Err(AppError::of(
            ErrorCode::Conflict,
            "This instance's Plex client identifier changed while the sign-in was in progress. \
             Start the sign-in again.",
        ));
    }

    // Every poll reaches plex.tv on the caller's behalf, so every poll counts
    // against the provider bucket — not just the call that created the pin.
    // A pin identifier is a bearer-free public string, so a limit spent only at
    // creation leaves anyone who has seen one able to drive unbounded traffic
    // at plex.tv from this instance's client identifier (PRD §21.4.3).
    spend_provider_budget(&state, client)?;

    match state
        .plex()
        .poll_pin(&attempt.plex_pin_id)
        .await
        .map_err(plex_failure)?
    {
        PinPoll::Pending => Ok((jar, Json(PinState::Pending))),
        PinPoll::Expired => {
            close(&state, &attempt.id, plex_pin::PinLoginResult::Expired, now).await;
            Ok((forget_attempt(jar, client), Json(PinState::Expired)))
        }
        PinPoll::Authorized { auth_token } => {
            // Whose token it is, asked before the attempt is consumed.
            //
            // This call reaches plex.tv, so it is the step that fails on a
            // timeout or a 5xx, and consuming the attempt first made one such
            // failure permanent: the row was stamped `consumed_at` with no
            // result, every later poll read it as closed, and the operator was
            // told a sign-in they had just completed had expired — with the
            // whole plex.tv exchange to start again (`I-UX-2`). Asked first, a
            // transient failure costs one 502 and the next poll retries.
            //
            // Two overlapping polls now both ask, which is one extra call to
            // plex.tv already counted against the provider budget above. What
            // must still happen once is everything after the claim.
            let account = state
                .plex()
                .account(&auth_token)
                .await
                .map_err(plex_failure)?;

            // Claimed before anything is stored and before a session exists.
            // Two overlapping polls are both told `Authorized` by plex.tv, and
            // without this both would store the token, refresh the account, and
            // issue a session — two valid sessions from one exchange. The claim
            // is a single serialised `consumed_at IS NULL` update, so exactly
            // one request gets past here.
            let claimed = state
                .database()
                .writer()
                .submit(plex_pin::ClaimPinLogin {
                    id: attempt.id.clone(),
                    at: now,
                })
                .await
                .map_err(AppError::internal)?;
            if !claimed {
                return Err(AppError::of(
                    ErrorCode::Conflict,
                    "That sign-in attempt has already been completed.",
                ));
            }
            authorize(
                &state,
                &attempt.id,
                PlexIdentity {
                    token: auth_token,
                    account,
                },
                client,
                &headers,
                jar,
                now,
            )
            .await
        }
    }
}

/// Whether this browser is the one that started attempt `id`.
///
/// A constant-time comparison would be beside the point — the identifier is in
/// the URL either way. What matters is that the value came back in a cookie
/// this instance set on the browser that started the exchange, which a
/// cross-site request cannot produce and `SameSite=Lax` withholds from a
/// cross-site `POST` even if it could.
fn holds_attempt(jar: &CookieJar, id: &str) -> bool {
    jar.get(PLEX_PIN_COOKIE)
        .is_some_and(|cookie| cookie.value() == id)
}

/// Clears the attempt cookie once the exchange is over, however it ended.
pub(super) fn forget_attempt(jar: CookieJar, client: ClientContext) -> CookieJar {
    jar.add(expire(PLEX_PIN_COOKIE, PLEX_PIN_COOKIE_PATH, client.scheme))
}

/// Spends one provider attempt, or refuses.
fn spend_provider_budget(state: &ApiState, client: ClientContext) -> AppResult<()> {
    state.limiter().spend(
        &Bucket::Provider,
        Some(client.address),
        "Too many calls to Plex from this address. Try again shortly.",
    )
}

pub(super) async fn close(
    state: &ApiState,
    id: &str,
    result: plex_pin::PinLoginResult,
    now: Timestamp,
) {
    let _ = state
        .database()
        .writer()
        .submit(plex_pin::CompletePinLogin {
            id: id.to_owned(),
            result,
            at: now,
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_poll_states_are_distinguishable_on_the_wire() {
        let pending = serde_json::to_value(PinState::Pending).expect("serialises");
        let expired = serde_json::to_value(PinState::Expired).expect("serialises");
        let authorized = serde_json::to_value(PinState::Authorized {
            user_id: "U".to_owned(),
            username: "operator".to_owned(),
            is_admin: false,
        })
        .expect("serialises");

        assert_eq!(pending["state"], "pending");
        assert_eq!(expired["state"], "expired");
        assert_eq!(authorized["state"], "authorized");
        assert_eq!(authorized["username"], "operator");
        // Every other body on this surface is camelCase, and the generated
        // client is built from the annotation rather than from a hand-written
        // guess at it (§24.5).
        assert_eq!(authorized["userId"], "U");
    }

    #[test]
    fn an_authorized_state_reports_the_privilege_rather_than_implying_one() {
        // The client routes on this. A linked account that administers nothing
        // reported as an administrator lands on pages that answer 403.
        let viewer = serde_json::to_value(PinState::Authorized {
            user_id: "U".to_owned(),
            username: "viewer".to_owned(),
            is_admin: false,
        })
        .expect("serialises");
        let administrator = serde_json::to_value(PinState::Authorized {
            user_id: "A".to_owned(),
            username: "operator".to_owned(),
            is_admin: true,
        })
        .expect("serialises");

        assert_eq!(viewer["isAdmin"], false);
        assert_eq!(administrator["isAdmin"], true);
    }

    #[test]
    fn an_attempt_is_only_completable_by_the_browser_holding_its_cookie() {
        use axum_extra::extract::cookie::Cookie;

        let started = CookieJar::new().add(Cookie::new(PLEX_PIN_COOKIE, "attempt-a"));
        assert!(holds_attempt(&started, "attempt-a"));
        // The identifier is public. Knowing it is not holding it.
        assert!(!holds_attempt(&started, "attempt-b"));
        assert!(!holds_attempt(&CookieJar::new(), "attempt-a"));
    }
}
