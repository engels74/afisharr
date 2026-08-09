// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Polling a plex.tv sign-in, and signing the operator in when it lands.

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use afisharr_core::{accounts, identifier::Id, plex_pin, secrets, time::Timestamp};
use afisharr_plex::{account::PlexAccount, pin::PinPoll};
use axum::{
    Json,
    extract::{Path, State},
    http::header::USER_AGENT,
};
use axum_extra::extract::CookieJar;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    authentication::{plex_pin_start::plex_failure, session},
    error::{AppError, AppResult, ErrorCode, Problem},
    proxy::ClientContext,
    ratelimit::{Bucket, Decision},
    state::ApiState,
};

/// The secret the Plex server token is stored under.
const PLEX_TOKEN_SECRET: &str = "plex.token";

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
    Authorized {
        /// The account now signed in.
        user_id: String,
        /// The account name.
        username: String,
    },
    /// The pin's window closed without a token.
    Expired,
}

/// Polls a Plex sign-in, and signs the operator in when the token arrives.
#[utoipa::path(
    get,
    path = "/api/auth/plex/pin/{id}",
    tag = "authentication",
    params(("id" = String, Path, description = "The attempt returned by the start call")),
    responses(
        (status = 200, description = "The attempt's current state", body = PinState),
        (status = 403, description = "The Plex account is not linked to an Afisharr account", body = Problem),
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
    let now = state.clock().now();
    let attempt = plex_pin::find_pin_login(state.database().readers(), &id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::of(ErrorCode::NotFound, "That sign-in attempt is not known."))?;

    if !attempt.is_open(now) {
        close(&state, &attempt.id, plex_pin::PinLoginResult::Expired, now).await;
        return Ok((jar, Json(PinState::Expired)));
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
            Ok((jar, Json(PinState::Expired)))
        }
        PinPoll::Authorized { auth_token } => {
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
            authorize(&state, &attempt.id, auth_token, client, &headers, jar, now).await
        }
    }
}

/// Spends one provider attempt, or refuses.
fn spend_provider_budget(state: &ApiState, client: ClientContext) -> AppResult<()> {
    match state
        .limiter()
        .record(&Bucket::Provider, Some(client.address))
    {
        Decision::Allowed => Ok(()),
        Decision::Refused {
            retry_after_seconds,
        } => Err(AppError::new(
            Problem::new(
                ErrorCode::RateLimited,
                "Too many calls to Plex from this address. Try again shortly.",
            )
            .retry_after(retry_after_seconds),
        )),
    }
}

/// Verifies whose token arrived, then stores it and mints the session.
///
/// The order is the whole security of this route. A completed pin exchange
/// proves that *somebody* holds a plex.tv account; it says nothing about who.
/// Signing in first and asking later would let anyone with a plex.tv account
/// walk into an instance that offers Plex sign-in — so the account is resolved,
/// matched against a linked row, and only then is the token worth storing.
///
/// Reached only by the request that claimed the attempt, so everything below
/// happens once. The `close` calls record how it ended on a row that is already
/// consumed.
async fn authorize(
    state: &ApiState,
    attempt_id: &str,
    auth_token: String,
    client: ClientContext,
    headers: &axum::http::HeaderMap,
    jar: CookieJar,
    now: Timestamp,
) -> AppResult<(CookieJar, Json<PinState>)> {
    let account = state
        .plex()
        .account(&auth_token)
        .await
        .map_err(plex_failure)?;

    let Some(user) = linked_account(state, &account).await? else {
        // Nothing is stored and no session is minted. The attempt is already
        // claimed, so recording the outcome is all that is left, and the same
        // pin cannot be replayed against a link created afterwards.
        close(state, attempt_id, plex_pin::PinLoginResult::Aborted, now).await;
        return Err(AppError::of(
            ErrorCode::Forbidden,
            "That Plex account is not linked to an Afisharr account.",
        ));
    };

    // The token goes to `secrets`, sealed, and never to `plex_pin_logins`
    // (PRD §19.6). It is the crown jewel: it authorises deletion.
    let sealed = state
        .secret_key()
        .seal(auth_token.as_bytes())
        .map_err(AppError::internal)?;
    state
        .database()
        .writer()
        .submit(secrets::PutSecret {
            name: PLEX_TOKEN_SECRET.to_owned(),
            sealed,
            at: now,
        })
        .await
        .map_err(AppError::internal)?;

    // The row is refreshed from plex.tv rather than left as it was: a renamed
    // account is the same account, matched on the numeric id, which is the
    // binding and not the key (P4).
    let refreshed = state
        .database()
        .writer()
        .submit(accounts::UpsertPlexUser {
            id: Id::generate(state.clock()),
            plex_account_id: account.id,
            plex_uuid: account.uuid.clone(),
            username: account.username.clone(),
            email: account.email.clone(),
            avatar_url: account.thumb.clone(),
            is_admin: user.is_admin,
            at: now,
        })
        .await
        .map_err(AppError::internal)?
        .map_err(AppError::internal)?;

    close(state, attempt_id, plex_pin::PinLoginResult::Success, now).await;

    let issued = session::issue(
        state,
        &refreshed.id,
        client,
        headers
            .get(USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
    )
    .await?;

    let mut jar = jar;
    for cookie in issued.cookies {
        jar = jar.add(cookie);
    }
    Ok((
        jar,
        Json(PinState::Authorized {
            user_id: refreshed.id.clone(),
            username: refreshed.username.clone(),
        }),
    ))
}

/// The `users` row this plex.tv account is bound to, if any.
///
/// Matched on `plex_account_id` — the binding plex.tv assigns — rather than on
/// the username, which the account holder can change at will. Tier 0 is an
/// admin-only surface (D-007), so there is no self-registration path here: an
/// account nobody has linked signs in as nobody.
async fn linked_account(
    state: &ApiState,
    account: &PlexAccount,
) -> AppResult<Option<accounts::User>> {
    Ok(
        accounts::find_by_plex_account(state.database().readers(), account.id)
            .await
            .map_err(AppError::internal)?
            .filter(accounts::User::is_active),
    )
}

async fn close(state: &ApiState, id: &str, result: plex_pin::PinLoginResult, now: Timestamp) {
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
        })
        .expect("serialises");

        assert_eq!(pending["state"], "pending");
        assert_eq!(expired["state"], "expired");
        assert_eq!(authorized["state"], "authorized");
        assert_eq!(authorized["username"], "operator");
    }
}
