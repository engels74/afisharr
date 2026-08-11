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
    http::{HeaderMap, header::HOST, request::Parts},
    middleware::{Next, from_fn_with_state},
    response::Response,
    routing::{delete, get, post},
};
use std::net::SocketAddr;

use crate::{
    authentication,
    error::AppError,
    files, health, interface, keys,
    proxy::{ClientContext, PublicOrigin, Scheme},
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
        // Every route in this group therefore declares the 429, for the reason
        // spelled out on [`require_setup_completed`]: a layer answers on behalf
        // of routes whose annotations are the sole contract the interface is
        // written against, and an answer no annotation declares is one the
        // interface simply does not handle.
        .layer(from_fn_with_state(state.clone(), anonymous_rate_limit))
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
        // before it spends anybody's budget, then the anonymous limit over
        // everything that survives. The limit is a layer and not a per-handler
        // call for the same reason the setup gate is — a route added later
        // inherits it without anybody remembering to (PRD §21.4.3).
        .layer(from_fn_with_state(state.clone(), anonymous_rate_limit))
        .layer(from_fn_with_state(state.clone(), require_setup_completed))
}

/// Counts a call that carries no credential against its address's budget.
///
/// The half of the API limit a layer can enforce, and only that half. A layer
/// runs before any extractor, so it knows what the request *presents* and never
/// whether the instance accepts it — and counting both kinds together is how an
/// unauthenticated flood came to spend the allowance the operator's own
/// interface needs. Behind a reverse proxy that `trustProxy` does not name,
/// every caller resolves to the proxy's address, so that flood held the whole
/// surface at 429 for the rest of the window from one source, with nothing in
/// the answer saying why.
///
/// The other half is [`crate::authentication::Authenticated`], which counts an
/// accepted credential against `Bucket::Api` under the credential's own name,
/// and a refused one against this same anonymous budget. Between them every
/// request through this group is counted exactly once.
async fn anonymous_rate_limit(
    State(state): State<ApiState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    if crate::authentication::presents_credential(request.headers()) {
        return Ok(next.run(request).await);
    }

    let client = request
        .extensions()
        .get::<ClientContext>()
        .copied()
        .ok_or_else(|| AppError::internal("the client context layer is not installed"))?;

    if let Decision::Refused {
        retry_after_seconds,
    } = state
        .limiter()
        .record(&Bucket::Anonymous, Some(client.address))
    {
        return Err(crate::ratelimit::too_many_requests(retry_after_seconds));
    }
    Ok(next.run(request).await)
}

/// The answer for a path under `/api` that no route claims.
///
/// It counts, and it has to count itself: a fallback sits under no route group,
/// so neither `protected_routes`' limit nor `setup_routes`' reaches it, and an
/// unmatched path was the one `/api` surface on every instance — configured or
/// not — that answered without being counted at all. The rule is
/// [`anonymous_rate_limit`]'s, applied by hand rather than restated: a request
/// that presents a credential is counted by the guard that judges it, and
/// everything else is counted here.
async fn unmatched_api_route(
    State(state): State<ApiState>,
    client: ClientContext,
    headers: HeaderMap,
) -> AppError {
    if !crate::authentication::presents_credential(&headers)
        && let Decision::Refused {
            retry_after_seconds,
        } = state
            .limiter()
            .record(&Bucket::Anonymous, Some(client.address))
    {
        return crate::ratelimit::too_many_requests(retry_after_seconds);
    }
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
    let mut client = ClientContext::resolve(peer, request.headers(), state.trusted_proxies());
    if reached_at_the_configured_origin(request.headers(), state.public_origin()) {
        client.scheme = Scheme::Https;
    }
    request.extensions_mut().insert(client);

    let mut response = next.run(request).await;
    security::apply_security_headers(
        response.headers_mut(),
        state.policy(),
        client.scheme,
        state.public_origin(),
    );
    response
}

/// Whether this request arrived at the `https` address the operator configured.
///
/// The gap this closes: `trustProxy` is empty by default, so a stock
/// deployment behind Caddy, nginx, or Cloudflare on an HTTPS address discards
/// the proxy's `X-Forwarded-Proto` and resolves as plaintext. The session
/// cookie is then set without `Secure` on a connection that is carrying TLS,
/// and nothing anywhere says the instance is in that state.
///
/// The operator's `publicOrigin` is the fix, because it is a statement about
/// this instance rather than about one request. Read together with the `Host`
/// the browser wrote from the URL it is calling, it says the request arrived at
/// the address the operator declared to be HTTPS — and it says so without
/// believing any header that decides its own answer.
///
/// It does not cover every deployment. A proxy that rewrites `Host` to the
/// upstream's own name — `proxy_pass` with no `proxy_set_header Host` — leaves
/// nothing here to match, and that instance still has to name its proxy in
/// `trustProxy` for the forwarded scheme to be honoured at all.
fn reached_at_the_configured_origin(
    headers: &HeaderMap,
    configured: Option<&PublicOrigin>,
) -> bool {
    let Some(origin) = configured.filter(|origin| origin.is_secure()) else {
        return false;
    };
    headers
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| origin.matches_host(host))
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
/// annotations against the router. Every route in `protected_routes` therefore
/// declares this 403 and the 429 from [`anonymous_rate_limit`], and a route
/// added to the group owes both.
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
