// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The whole HTTP surface, in the order a request crosses it.
//!
//! Routes are grouped by the six primary destinations plus settings (PRD §6.1),
//! because that is how the operator thinks about them and it is what keeps the
//! generated client's tags meaningful. The middleware order is the security
//! model, and it is stated once here rather than reasoned about per route.
//!
//! The one thing that is not here is [`limits`], which holds the two rate-limit
//! layers and the two fallbacks that count themselves. Which limit a group
//! carries is part of the route table and stays below; *what counting means*
//! is one rule read four ways, and it is easier to keep whole in one place than
//! to rediscover per group.

mod limits;

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
    authentication, error::AppError, files, health, interface, keys, proxy::ClientContext,
    security, setup, state::ApiState, stream,
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
        .fallback(limits::unmatched_api_route);

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
///
/// It carries a limit like every other group. Answering without a credential is
/// not the same as answering without being counted, and this was the one `/api`
/// path metered by nothing — against a module whose opening rule is that every
/// request through `/api` is counted exactly once. An unauthenticated caller
/// looping it was answered as fast as the instance accepts connections while
/// `Bucket::Anonymous` read zero for their address, so the budget was untouched
/// when they moved to a metered route and an operator investigating a saturated
/// box found every counter reporting no traffic at all.
///
/// [`limits::every_call`], for the reason [`setup_routes`] gives: no handler
/// here constructs [`crate::authentication::Authenticated`], so
/// [`limits::anonymous`]'s waiver would have no keeper. The allowance is 300 a
/// minute per address, which no orchestrator's liveness probe comes near.
fn open_routes(state: &ApiState) -> Router<ApiState> {
    Router::new()
        .route("/health", get(health::health))
        .layer(from_fn_with_state(state.clone(), limits::every_call))
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
        // Read outside-in, exactly as `protected_routes` reads: the setup gate
        // first, so a configured instance refuses before it spends anybody's
        // budget, then the anonymous limit over everything that survives.
        //
        // Nothing in this group carries a credential — the claim is a cookie,
        // and `GET /setup/claim` is answered before one exists — so without the
        // limit here the whole surface a freshly deployed container leaves open
        // was uncounted. Each of those calls runs two reader-pool queries, so a
        // caller looping one path held the reader pool against the operator's
        // own claim page and the instance could not be claimed at all
        // (PRD §21.4.3).
        //
        // [`limits::every_call`] and not [`limits::anonymous`], for that
        // same reason read the other way round: the anonymous layer waives
        // itself for a request that presents a credential, on the promise that
        // the guard behind it counts that request instead. No route here
        // constructs [`crate::authentication::Authenticated`], so there is no
        // guard to keep the promise, and the layer's waiver was a way to opt
        // out of the limit entirely by sending a header — `Authorization:
        // Bearer x` on the claim path, looped, metered by nothing.
        //
        // Every route in this group therefore declares the 429, for the reason
        // spelled out on [`require_setup_completed`]: a layer answers on behalf
        // of routes whose annotations are the sole contract the interface is
        // written against, and an answer no annotation declares is one the
        // interface simply does not handle.
        .layer(from_fn_with_state(state.clone(), limits::every_call))
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
///
/// Two groups under that one gate, and the split is the rate limit's rather
/// than the setup gate's: [`limits::anonymous`]'s waiver is only sound over
/// routes that construct [`crate::authentication::Authenticated`], and the
/// sign-in routes construct none.
fn protected_routes(state: &ApiState) -> Router<ApiState> {
    Router::new()
        .merge(sign_in_routes(state))
        .merge(guarded_routes(state))
        .layer(from_fn_with_state(state.clone(), require_setup_completed))
}

/// The routes that hand a credential out rather than presenting one.
///
/// [`limits::every_call`] and not [`limits::anonymous`], for the reason
/// [`setup_routes`] gives: no handler here takes
/// [`crate::authentication::Authenticated`], so there is no guard behind the
/// waiver to keep its promise. Each of the three also has an early return that
/// reaches no bucket of its own — a body `JsonBody` refuses, a `forwardUrl`
/// this instance will not stand behind, an attempt cookie that does not name
/// the identifier in the path — so `Authorization: Bearer x` was answered as
/// fast as the instance accepts connections while every counter read zero.
fn sign_in_routes(state: &ApiState) -> Router<ApiState> {
    Router::new()
        .route("/auth/login", post(authentication::log_in))
        .route("/auth/plex/pin", post(authentication::start_plex_pin))
        // `post`, not `get`: completing a pin consumes the attempt, stores a
        // token, and sets a session cookie. A `GET` is what a cross-site
        // navigation and a prefetch can reach, and the CSRF layer exempts
        // every safe method — so a read-shaped route here is a login state
        // change with no protection over it.
        .route("/auth/plex/pin/{id}", post(authentication::poll_plex_pin))
        .layer(from_fn_with_state(state.clone(), limits::every_call))
}

/// The routes a credential guard stands behind.
///
/// Every handler here takes [`crate::authentication::Authenticated`] or
/// [`crate::authentication::Administrator`], which is what makes
/// [`limits::anonymous`]'s waiver sound: a credentialled request is counted
/// by the extractor that judges it, and one presenting none is counted here.
fn guarded_routes(state: &ApiState) -> Router<ApiState> {
    Router::new()
        .route("/auth/logout", post(authentication::log_out))
        .route("/auth/session", get(authentication::whoami))
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
        // Called after the last route and before the layer, which is what makes
        // it work at all: it attaches to every method router registered so far,
        // and it sits *inside* the limit so the request it answers is one the
        // limit has already seen. That is also why it counts only the
        // credentialled half — the other half the layer already counted, and
        // counting it twice made one wrong-method request spend two attempts.
        //
        // Without it, [`limits::anonymous`]'s waiver had no keeper on a
        // wrong-method request. Axum answers 405 from the method router, before
        // any handler extractor runs, so no `Authenticated` was ever
        // constructed and nothing counted the call — while the layer had waived
        // it for merely *presenting* something. `presents_credential` reads a
        // header and validates nothing, so `Cookie: afisharr_session=x` against
        // `GET /api/auth/logout`, looped, was answered as fast as the instance
        // accepts connections with `Bucket::Anonymous` and `Bucket::Api` both
        // reading zero. This is the hole `limits::unmatched_api_route` closes for the
        // fallback, left open for every matched path in this group.
        .method_not_allowed_fallback(limits::guarded_method_not_allowed)
        // The limit is a layer and not a per-handler call for the same reason
        // the setup gate is — a route added later inherits it without anybody
        // remembering to (PRD §21.4.3).
        .layer(from_fn_with_state(state.clone(), limits::anonymous))
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
    let client = ClientContext::resolve(peer, request.headers(), state.trusted_proxies())
        .at_configured_origin(request.headers(), state.public_origin());
    request.extensions_mut().insert(client);

    let mut response = next.run(request).await;
    security::apply_security_headers(
        response.headers_mut(),
        state.policy(),
        client,
        state.public_origin(),
    );
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
    State(state): State<ApiState>,
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
        state.public_origin(),
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
///
/// A layer answers on behalf of every route under it, and the generated client
/// is built from each route's own `#[utoipa::path(responses(...))]` block — so
/// an answer produced here that no annotation declares is an answer the sole
/// contract between the two surfaces says cannot happen. That is not a
/// documentation gap: the interface is written against the contract, so the
/// case simply goes unhandled, and `contract-check` stays green while it does,
/// because it checks the client against the annotations and never the
/// annotations against the router. Every route in [`protected_routes`]
/// therefore declares this 403 and the 429 its group's rate limit produces —
/// [`limits::anonymous`] for [`guarded_routes`], [`limits::every_call`]
/// for [`sign_in_routes`] — and a route added to either owes both.
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
