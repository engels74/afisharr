// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What counts a request, and what the surface answers when nothing routes it.
//!
//! Apart from the route tables because it is one rule read four ways, and the
//! rule is easier to keep whole than to rediscover per group: **every request
//! through `/api` is counted exactly once**. Two layers and two fallbacks are
//! all there is to it, and the difference between them is only ever whether
//! something *behind* them will do the counting instead.

use axum::{
    body::Body,
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{
    error::AppError,
    proxy::ClientContext,
    ratelimit::{Bucket, Decision},
    state::ApiState,
};

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
/// request through that group is counted exactly once.
///
/// The waiver is only sound where that other half actually runs. A group whose
/// routes take no [`crate::authentication::Authenticated`] must use
/// [`every_call`], or a header is all it takes to opt out of the limit.
/// `guarded_routes` is the one group this belongs over; a route added to it
/// that takes no credential guard belongs in `sign_in_routes` instead.
///
/// It is also only sound where a handler actually runs, which is what
/// [`guarded_method_not_allowed`] exists for.
pub(super) async fn anonymous(
    State(state): State<ApiState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    if crate::authentication::presents_credential(request.headers()) {
        return Ok(next.run(request).await);
    }
    count_against_address(&state, &request)?;
    Ok(next.run(request).await)
}

/// Counts every call in a group against its address's budget, credential or not.
///
/// [`anonymous`] with the waiver removed, for the groups that have nothing to
/// waive it to. There are two: `setup_routes`, whose handlers take a `CookieJar`
/// and a `ClientContext` and never the credential guard, and `sign_in_routes`,
/// whose handlers are how a caller obtains a credential in the first place. In
/// both, the promise "a request that presents a credential is counted by the
/// guard that judges it" has no keeper, and an unauthenticated caller who sent
/// an invented `Authorization` header was counted by nothing at all.
///
/// A credential is not a thing those groups have any use for, so counting one
/// against its address costs a real caller nothing: the operator claiming their
/// own instance sends no `Authorization` header, and the browser holding the
/// claim sends a cookie this does not read.
pub(super) async fn every_call(
    State(state): State<ApiState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    count_against_address(&state, &request)?;
    Ok(next.run(request).await)
}

/// The answer for a method a guarded route does not serve.
///
/// It counts the half [`anonymous`] waived, and only that half. This sits
/// *inside* that layer, so a request presenting no credential has already been
/// counted by it on the way in; counting again here spent two attempts for one
/// request, and the module's own rule — every request through `/api` counted
/// exactly once — was false for this path. A scanner probing wrong methods
/// therefore drained the shared 300-per-minute allowance at twice the rate,
/// and the operator's own sign-in answered 429 inside a window it still had
/// budget in.
///
/// What is left to count is the credentialled request, which is where the
/// waiver has no keeper: axum answers 405 from the method router before any
/// extractor runs, so no `Authenticated` is ever constructed.
/// `presents_credential` reads a header and validates nothing, so
/// `Cookie: afisharr_session=x` against `GET /api/auth/logout`, looped, was
/// answered as fast as the instance accepts connections with `Bucket::Anonymous`
/// and `Bucket::Api` both reading zero.
///
/// Without it, [`anonymous`]'s waiver had no keeper on a wrong-method request.
/// Axum answers 405 from the method router, before any handler extractor runs,
/// so no `Authenticated` was ever constructed and nothing counted the call —
/// while the layer had waived it for merely *presenting* something.
/// `presents_credential` reads a header and validates nothing, so
/// `Cookie: afisharr_session=x` against `GET /api/auth/logout`, looped, was
/// answered as fast as the instance accepts connections with
/// `Bucket::Anonymous` and `Bucket::Api` both reading zero.
///
/// The status and the empty body are axum's own, deliberately. A `Problem` body
/// needs a code for "method not allowed", [`crate::error::ErrorCode`] is a
/// closed set whose every variant is part of the generated client's contract,
/// and adding one is a contract change rather than a fix (§24.5).
pub(super) async fn guarded_method_not_allowed(
    State(state): State<ApiState>,
    client: ClientContext,
    headers: axum::http::HeaderMap,
) -> Response {
    if !crate::authentication::presents_credential(&headers) {
        // Already counted by the layer this sits inside.
        return axum::http::StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    match spend_anonymous_budget(&state, client) {
        Some(refusal) => refusal.into_response(),
        None => axum::http::StatusCode::METHOD_NOT_ALLOWED.into_response(),
    }
}

/// The answer for a path under `/api` that no route claims.
///
/// It counts, and it has to count itself: a fallback sits under no route group,
/// so neither `protected_routes`' limit nor `setup_routes`' reaches it, and an
/// unmatched path was the one `/api` surface on every instance — configured or
/// not — that answered without being counted at all.
///
/// It counts every call, [`every_call`]'s rule and not [`anonymous`]'s, for the
/// reason [`guarded_method_not_allowed`] gives. Applying the waiver here meant
/// `Authorization: Bearer x` against any unmatched path answered for ever
/// without incrementing any bucket, while the logs filled with refusals and
/// every counter read zero.
pub(super) async fn unmatched_api_route(
    State(state): State<ApiState>,
    client: ClientContext,
) -> AppError {
    spend_anonymous_budget(&state, client).unwrap_or_else(|| {
        AppError::of(
            crate::error::ErrorCode::NotFound,
            "No such endpoint on this instance.",
        )
    })
}

/// Spends one anonymous attempt for a fallback, and reports the refusal.
///
/// One reader for both fallbacks. They differ only in what they answer when the
/// budget holds, and stating the counting twice is two chances for one of them
/// to stop counting without anybody noticing.
fn spend_anonymous_budget(state: &ApiState, client: ClientContext) -> Option<AppError> {
    match state
        .limiter()
        .record(&Bucket::Anonymous, Some(client.address))
    {
        Decision::Refused {
            retry_after_seconds,
        } => Some(crate::ratelimit::too_many_requests(retry_after_seconds)),
        Decision::Allowed => None,
    }
}

/// Records one call against the anonymous budget of the address it came from.
///
/// The address is the resolved one, so a request behind a trusted proxy is
/// counted against the client rather than the proxy. A missing context is a
/// wiring fault and not a request anybody can fix, which is why it is refused
/// rather than counted against nobody.
fn count_against_address(state: &ApiState, request: &Request<Body>) -> Result<(), AppError> {
    let client = request
        .extensions()
        .get::<ClientContext>()
        .copied()
        .ok_or_else(|| AppError::internal("the client context layer is not installed"))?;

    match crate::authentication::spend_anonymous(state, Some(client.address)) {
        Some(refusal) => Err(refusal),
        None => Ok(()),
    }
}
