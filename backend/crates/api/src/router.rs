// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The whole HTTP surface, in one file, in the order a request crosses it.
//!
//! Routes are grouped by the six primary destinations plus settings (PRD §6.1),
//! because that is how the operator thinks about them and it is what keeps the
//! generated client's tags meaningful. The middleware order is the security
//! model, and it is stated once here rather than reasoned about per route.

use axum::{
    Router,
    body::Body,
    extract::{ConnectInfo, FromRequestParts, Request, State},
    http::request::Parts,
    middleware::{Next, from_fn_with_state},
    response::Response,
    routing::{delete, get, post},
};
use std::net::SocketAddr;

use crate::{
    authentication,
    error::AppError,
    files, health, interface, keys,
    proxy::ClientContext,
    ratelimit::{Bucket, Decision},
    security, setup,
    state::ApiState,
    stream,
};

/// Builds the router for `state`.
///
/// Read outside-in, the layers are: the security headers every answer carries,
/// the resolved client context every gate is keyed on, and CSRF. Nothing below
/// them can opt out, which is the point — `I-SEC-2` fails the moment a header
/// is a handler's responsibility.
pub fn build(state: ApiState) -> Router {
    let api = Router::new()
        .merge(open_routes(&state))
        .merge(setup_routes(&state))
        .merge(protected_routes(&state))
        // An unmatched `/api` path answers the one shape too. Axum's default
        // is an empty 404 body, which is a response the generated client
        // cannot narrow and the interface has to guess at (`I-UX-2`).
        .fallback(unmatched_api_route);

    Router::new()
        .nest("/api", api)
        // Anything not under /api is a page, and the SPA routes it itself.
        .fallback(get(interface::spa))
        .layer(from_fn_with_state(state.clone(), csrf))
        .layer(from_fn_with_state(state.clone(), envelope))
        .with_state(state)
}

/// The routes that answer without a credential.
///
/// Exactly one, and it is named in `I-SEC-8`'s statement: health. Anything
/// added here is a hole in the first-run rule, which is why the list is short
/// enough to read.
fn open_routes(_state: &ApiState) -> Router<ApiState> {
    Router::new().route("/health", get(health::health))
}

/// The wizard, behind the claim gate.
///
/// `claim` and `recover` are outside the gate — they are how the gate is
/// passed, and `GET /setup/claim` is what the claim page renders itself from
/// before it has one. Everything else is inside it.
fn setup_routes(state: &ApiState) -> Router<ApiState> {
    let gated = Router::new()
        .route("/setup/status", get(setup::status))
        .route("/setup/admin", post(setup::create_admin))
        .route("/setup/complete", post(setup::complete))
        .layer(from_fn_with_state(state.clone(), setup::require_claim));

    Router::new()
        .route("/setup/claim", get(setup::claim_status).post(setup::claim))
        .route("/setup/recover", post(setup::recover))
        .merge(gated)
        .layer(from_fn_with_state(
            state.clone(),
            setup::require_setup_incomplete,
        ))
}

/// Everything else, refused until setup is finished.
///
/// The refusal is a layer over the whole group rather than a check inside each
/// handler: an unconfigured instance grants nothing, and a route added later
/// inherits that without anybody remembering to (`I-SEC-8`).
fn protected_routes(state: &ApiState) -> Router<ApiState> {
    Router::new()
        .route("/auth/login", post(authentication::log_in))
        .route("/auth/logout", post(authentication::log_out))
        .route("/auth/session", get(authentication::whoami))
        .route("/auth/plex/pin", post(authentication::start_plex_pin))
        // `post`, not `get`: completing a pin consumes the attempt, stores a
        // token, and sets a session cookie. A `GET` is what a cross-site
        // navigation and a prefetch can reach, and the CSRF layer exempts
        // every safe method — so a read-shaped route here is a login state
        // change with no protection over it.
        .route("/auth/plex/pin/{id}", post(authentication::poll_plex_pin))
        .route("/files", get(files::browse))
        .route("/files/roots", get(files::roots))
        .route("/settings/password", post(authentication::change_password))
        .route("/settings/sessions", get(authentication::list_sessions))
        .route(
            "/settings/sessions/{id}",
            delete(authentication::revoke_session),
        )
        .route("/settings/api-keys", get(keys::list).post(keys::create))
        .route("/settings/api-keys/{id}", delete(keys::revoke))
        .route("/stream", get(stream::stream))
        // Read outside-in: setup first, so an unconfigured instance refuses
        // before it spends anybody's budget, then the API limit over everything
        // that survives. The limit is a layer and not a per-handler call for
        // the same reason the setup gate is — a route added later inherits it
        // without anybody remembering to (PRD §21.4.3).
        .layer(from_fn_with_state(state.clone(), api_rate_limit))
        .layer(from_fn_with_state(state.clone(), require_setup_completed))
}

/// Counts every call to a protected route against the caller's API budget.
///
/// `Bucket::Api` is 600 requests a minute (PRD §21.4.3). Without this it is a
/// table entry with no enforcement: a caller holding a valid session or key
/// could drive the database, the filesystem browser, and the stream as fast as
/// the process answers, indefinitely.
async fn api_rate_limit(
    State(state): State<ApiState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let client = request
        .extensions()
        .get::<ClientContext>()
        .copied()
        .ok_or_else(|| AppError::internal("the client context layer is not installed"))?;

    if let Decision::Refused {
        retry_after_seconds,
    } = state.limiter().record(&Bucket::Api, Some(client.address))
    {
        return Err(AppError::new(
            crate::error::Problem::new(
                crate::error::ErrorCode::RateLimited,
                "Too many requests from this address. Try again shortly.",
            )
            .retry_after(retry_after_seconds),
        ));
    }
    Ok(next.run(request).await)
}

/// The answer for a path under `/api` that no route claims.
async fn unmatched_api_route() -> AppError {
    AppError::of(
        crate::error::ErrorCode::NotFound,
        "No such endpoint on this instance.",
    )
}

/// Resolves who the request is from, then writes the header set on the answer.
///
/// One layer for both because they are two halves of the same fact: the
/// resolved scheme decides `Strict-Transport-Security` and the `Secure` cookie
/// flag, and resolving it twice would be two chances to disagree (P7).
async fn envelope(
    State(state): State<ApiState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let client = ClientContext::resolve(peer, request.headers(), state.trusted_proxies());
    request.extensions_mut().insert(client);

    let mut response = next.run(request).await;
    security::apply_security_headers(response.headers_mut(), state.policy(), client.scheme);
    response
}

/// Refuses a state-changing request that another site could have caused.
///
/// Enumerating the ambient credentials is this layer's job, not the judge's:
/// the judge decides, and the perimeter says what a browser could have been
/// made to attach. There are three. The session cookie is the obvious one; the
/// setup claim is the one that is easy to miss, and behind it sit the routes
/// that create the administrator and finish setup; the Plex attempt cookie is
/// the third, and behind it sits the request that turns a finished plex.tv
/// exchange into a session.
async fn csrf(
    State(_state): State<ApiState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    use axum_extra::extract::CookieJar;

    let jar = CookieJar::from_headers(request.headers());
    let carries_ambient_credential = jar.get(security::SESSION_COOKIE).is_some()
        || jar.get(afisharr_core::setup::CLAIM_COOKIE).is_some()
        || jar.get(security::PLEX_PIN_COOKIE).is_some();
    let token = jar
        .get(security::CSRF_COOKIE)
        .map(|cookie| cookie.value().to_owned());

    match security::judge_csrf(
        request.method(),
        request.headers(),
        token.as_deref(),
        carries_ambient_credential,
    ) {
        security::CsrfDecision::Allowed => Ok(next.run(request).await),
        security::CsrfDecision::ForeignOrigin => Err(AppError::of(
            crate::error::ErrorCode::Forbidden,
            "That request came from another site and was refused.",
        )),
        security::CsrfDecision::TokenMismatch => Err(AppError::of(
            crate::error::ErrorCode::Forbidden,
            "That request was missing its cross-site protection token. Reload and try again.",
        )),
    }
}

/// Refuses everything while `instance.setup_completed_at` is `NULL`.
async fn require_setup_completed(
    State(state): State<ApiState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    if state.setup_completed() {
        return Ok(next.run(request).await);
    }
    Err(AppError::of(
        crate::error::ErrorCode::SetupRequired,
        "This instance has not been set up. Claim it with the token printed on the console.",
    ))
}

impl FromRequestParts<ApiState> for ClientContext {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &ApiState,
    ) -> Result<Self, Self::Rejection> {
        // Inserted by `envelope`, which wraps the whole router. A handler
        // reaching this and finding nothing means the layer was removed, which
        // is a wiring bug and not a request the caller can fix.
        parts
            .extensions
            .get::<Self>()
            .copied()
            .ok_or_else(|| AppError::internal("the client context layer is not installed"))
    }
}
