// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Turning a finished plex.tv exchange into a session.
//!
//! Reached only by the request that claimed the attempt, so everything here
//! happens exactly once per exchange.

use afisharr_core::{accounts, identifier::Id, plex_pin, secrets, time::Timestamp};
use afisharr_plex::account::PlexAccount;
use axum::{Json, http::header::USER_AGENT};
use axum_extra::extract::CookieJar;

use crate::{
    authentication::{
        plex_pin_poll::{PinState, close, forget_attempt},
        session,
    },
    error::{AppError, AppResult, ErrorCode},
    proxy::ClientContext,
    state::ApiState,
};

/// The secret the Plex server token is stored under.
const PLEX_TOKEN_SECRET: &str = "plex.token";

/// What a finished plex.tv exchange yielded, and whose it is.
///
/// One value because the two halves are one answer: the token is only ever
/// used on behalf of the account that `/user` resolved it to, and passing them
/// separately invites a call site that has one without the other.
pub(super) struct PlexIdentity {
    /// The token plex.tv issued for the completed pin.
    pub token: String,
    /// The account plex.tv says it belongs to.
    pub account: PlexAccount,
}

/// Verifies whose token arrived, then mints the session.
///
/// The order is the whole security of this route. A completed pin exchange
/// proves that *somebody* holds a plex.tv account; it says nothing about who.
/// Signing in first and asking later would let anyone with a plex.tv account
/// walk into an instance that offers Plex sign-in — so the account is resolved,
/// matched against a linked row, and only then is anything done with the token.
///
/// Reached only by the request that claimed the attempt, so everything below
/// happens once. The `close` calls record how it ended on a row that is already
/// consumed.
///
/// `identity` is resolved by the caller, before the claim. Resolving it is the
/// one step that reaches plex.tv, and a transient failure inside a consumed
/// attempt is a completed sign-in the operator can never finish — so the call
/// that can fail happens while the attempt is still open (see `plex_pin_poll`).
pub(super) async fn authorize(
    state: &ApiState,
    attempt_id: &str,
    identity: PlexIdentity,
    client: ClientContext,
    headers: &axum::http::HeaderMap,
    jar: CookieJar,
    now: Timestamp,
) -> AppResult<(CookieJar, Json<PinState>)> {
    let PlexIdentity {
        token: auth_token,
        account,
    } = identity;

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

    // `plex.token` is one secret for the whole instance, and it is the
    // credential every server operation runs under — it authorises deletion.
    // A linked viewer signing in is not the integration's owner, and writing
    // their token here would quietly downgrade what Afisharr can do in Plex
    // for everybody, at the moment somebody who administers nothing signed in.
    // So the session is issued either way and the credential is replaced only
    // by an account that administers this instance (PRD §19.6).
    if user.is_admin {
        // Sealed, and never to `plex_pin_logins`.
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
    }

    // The row is refreshed from plex.tv rather than left as it was: a renamed
    // account is the same account, matched on the numeric id, which is the
    // binding and not the key (P4). The local username survives a rename that
    // would collide with another account's, so a sign-in whose pin is already
    // spent cannot fail on a name plex.tv chose.
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
        .map_err(|error| match error {
            // Only reachable for an account no row is bound to yet, which is
            // not this route: the link was found above. Defined anyway, so a
            // name that is already taken is never a 500 an operator has to
            // read a log to understand.
            accounts::AccountError::UsernameTaken { ref username } => AppError::of(
                ErrorCode::Conflict,
                format!("Another account on this instance already uses the name '{username}'."),
            )
            .caused_by(error),
            other => AppError::internal(other),
        })?;

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
        forget_attempt(jar, client),
        Json(PinState::Authorized {
            user_id: refreshed.id.clone(),
            username: refreshed.username.clone(),
            is_admin: refreshed.is_admin,
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
