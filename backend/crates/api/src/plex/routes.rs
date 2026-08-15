// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `/api/settings/plex/connection`.

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use axum::{Json, extract::State};

use crate::{
    authentication::{Administrator, PlexRead},
    error::{AppResult, Problem},
    plex::{check, connection::PlexConnection},
    ratelimit::Bucket,
    state::ApiState,
};

/// Checks the Plex connection and reports what it saw.
///
/// A `POST` because it is an act with a side effect, not a read of a cached
/// value: it makes a request to the operator's server and records the
/// observation. Answering it from a `GET` would put a network call and a write
/// behind a method a browser prefetch may issue, and behind a method the CSRF
/// layer exempts.
///
/// Metered against the provider bucket rather than the general API one: it
/// reaches somebody else's server on the caller's behalf, which is the rule
/// PRD §21.4.3 sets for every provider-calling endpoint.
#[utoipa::path(
    post,
    path = "/api/settings/plex/connection/check",
    tag = "settings",
    responses(
        (status = 200, description = "What the check saw. A `wrongServer` state is blocking (`I-ID-5`) and carries both identifiers", body = PlexConnection),
        (status = 401, description = "No accepted credential was presented", body = Problem),
        (status = 403, description = "That account does not administer this instance, that key was not issued with the scope this route needs, or setup has not been completed", body = Problem),
        (status = 429, description = "Too many requests", body = Problem),
    ),
)]
pub async fn check_connection(
    State(state): State<ApiState>,
    client: crate::proxy::ClientContext,
    _caller: Administrator<PlexRead>,
) -> AppResult<Json<PlexConnection>> {
    state.limiter().spend(
        &Bucket::Provider,
        Some(client.address),
        "Too many checks against Plex. Try again shortly.",
    )?;
    Ok(Json(check::run(&state).await?))
}
