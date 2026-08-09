// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Starting a plex.tv PIN or OAuth sign-in.

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use afisharr_core::{identifier::Id, plex_pin};
use afisharr_plex::pin::{AuthorizationUrl, Mode, PinError};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, header::HOST},
};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use url::Url;
use utoipa::ToSchema;

use crate::{
    authentication::session,
    error::{AppError, AppResult, ErrorCode, JsonBody, Problem},
    proxy::{ClientContext, Scheme},
    ratelimit::{Bucket, Decision},
    security::{PLEX_PIN_COOKIE, PLEX_PIN_COOKIE_PATH, set},
    state::ApiState,
};

/// What the interface asks for when it starts a Plex sign-in.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartPin {
    /// `pin` shows a four-character code; `oauth` sends the operator to
    /// plex.tv's hosted sign-in.
    pub oauth: bool,
    /// Where plex.tv returns the operator, for the OAuth variant.
    ///
    /// It must name this instance. plex.tv redirects to whatever this asks
    /// for, so a target anywhere else turns the endpoint into a redirector
    /// wearing a legitimate `app.plex.tv/auth` URL, and the operator who
    /// completes the sign-in lands on somebody else's page.
    pub forward_url: Option<String>,
}

/// A started sign-in.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PinStarted {
    /// Afisharr's identifier for this attempt. The client polls this.
    pub id: String,
    /// The four-character code, for the PIN variant.
    pub code: String,
    /// Where to send the operator, for the OAuth variant.
    pub authorization_url: Option<String>,
    /// When plex.tv stops answering for this pin, in epoch milliseconds.
    pub expires_at: i64,
}

/// Starts a Plex sign-in.
#[utoipa::path(
    post,
    path = "/api/auth/plex/pin",
    tag = "authentication",
    request_body = StartPin,
    responses(
        (status = 200, description = "A pin was created", body = PinStarted),
        (status = 400, description = "The request body was not readable, or the return target is not this instance", body = Problem),
        (status = 429, description = "Too many attempts", body = Problem),
        (status = 502, description = "plex.tv could not be reached", body = Problem),
    ),
)]
pub async fn start_plex_pin(
    State(state): State<ApiState>,
    client: ClientContext,
    jar: CookieJar,
    headers: HeaderMap,
    JsonBody(request): JsonBody<StartPin>,
) -> AppResult<(CookieJar, Json<PinStarted>)> {
    // A pin creation reaches plex.tv on the caller's behalf, so it counts
    // against the provider bucket rather than the general API one (§21.4.3).
    if let Decision::Refused {
        retry_after_seconds,
    } = state
        .limiter()
        .record(&Bucket::Provider, Some(client.address))
    {
        return Err(AppError::new(
            Problem::new(
                ErrorCode::RateLimited,
                "Too many sign-in attempts against Plex. Try again shortly.",
            )
            .retry_after(retry_after_seconds),
        ));
    }

    let mode = if request.oauth {
        Mode::OAuth
    } else {
        Mode::Pin
    };

    // Judged before plex.tv is called and before a row is stored: a return
    // target this instance will not stand behind is a bad request, not a
    // reason to spend an upstream call and leave an attempt behind.
    let forward_to = match (mode, request.forward_url.as_deref()) {
        (Mode::OAuth, Some(forward_to)) if returns_here(forward_to, client.scheme, &headers) => {
            Some(forward_to)
        }
        (Mode::OAuth, Some(_)) => {
            return Err(AppError::new(
                Problem::new(
                    ErrorCode::Invalid,
                    "A Plex sign-in can only return to this instance.",
                )
                .at("/forwardUrl"),
            ));
        }
        _ => None,
    };
    let resource = state
        .plex()
        .create_pin(request.oauth)
        .await
        .map_err(plex_failure)?;

    let now = state.clock().now();
    let expires_at = now.plus_millis(resource.expires_in_seconds.saturating_mul(1000));
    let stored = state
        .database()
        .writer()
        .submit(plex_pin::RecordPinLogin {
            id: Id::generate(state.clock()),
            plex_pin_id: resource.plex_pin_id,
            code: resource.code.clone(),
            mode: mode.as_text(),
            client_identifier: resource.client_identifier,
            at: now,
            expires_at,
        })
        .await
        .map_err(AppError::internal)?;

    let authorization_url = match forward_to {
        Some(forward_to) => Some(
            AuthorizationUrl::build(state.plex().identity(), &resource.code, forward_to)
                .map_err(AppError::internal)?
                .as_str()
                .to_owned(),
        ),
        None => None,
    };

    // Two cookies, and neither of them signs anybody in. The first binds this
    // attempt to this browser, so completing it is something only the browser
    // that started it can do; the second is the token that completion has to
    // echo, because an attempt cookie is an ambient credential and every
    // ambient credential is judged (PRD §21.4.2).
    let jar = jar
        .add(set(
            PLEX_PIN_COOKIE,
            stored.id.clone(),
            PLEX_PIN_COOKIE_PATH,
            now.millis_until(stored.expires_at) / 1000,
            client.scheme,
            true,
        ))
        .add(session::csrf_cookie(client.scheme));

    Ok((
        jar,
        Json(PinStarted {
            id: stored.id,
            code: stored.code,
            authorization_url,
            expires_at: stored.expires_at.as_millis(),
        }),
    ))
}

/// Whether `forward_to` sends the operator back to this instance.
///
/// The origin is taken from the request rather than from the body, because the
/// body is the attacker's half of this: plex.tv redirects to whatever the
/// `forwardUrl` names, and it does so from a real `app.plex.tv/auth` URL that
/// this endpoint minted, so an unchecked target is an open redirect signed by
/// Afisharr. Scheme comes from the resolved client context — the one place
/// that decides whether a forwarded `https` is believable (`I-SEC-1`) — and the
/// authority from `Host`, which is the same value the CSRF check binds a
/// declared `Origin` to (P7).
///
/// Origins are compared as origins, not as strings: `https://host` and
/// `https://host:443` are one instance, and a target with an opaque origin —
/// `javascript:`, `data:` — is not one at all.
fn returns_here(forward_to: &str, scheme: Scheme, headers: &HeaderMap) -> bool {
    let Some(host) = headers.get(HOST).and_then(|value| value.to_str().ok()) else {
        // Nothing to compare against, so nothing is provably this instance.
        return false;
    };
    // A `Host` carrying a path, a query, or userinfo is not an authority, and
    // parsing it as one would let the extra part choose the origin.
    if host.is_empty() || host.contains(['/', '\\', '@', '?', '#']) {
        return false;
    }
    let (Ok(here), Ok(target)) = (
        Url::parse(&format!("{}://{host}", scheme.as_str())),
        Url::parse(forward_to),
    ) else {
        return false;
    };
    target.origin().is_tuple() && target.origin() == here.origin()
}

/// Renders a plex.tv failure in the operator's terms.
///
/// The status matters as much as the sentence. Both plex.tv routes document a
/// transport failure as 502, and `Internal` maps to 500 — a status the
/// operation never declared, and one that tells a generated client nothing
/// about whether the fault is upstream or here (§24.5).
pub(crate) fn plex_failure(error: PinError) -> AppError {
    match &error {
        PinError::ClientIdentifierMismatch { .. } => AppError::of(
            ErrorCode::Conflict,
            "plex.tv issued the sign-in under a different client identifier. \
             Start the sign-in again.",
        ),
        // A response this build cannot follow is still plex.tv answering with
        // something unusable, not Afisharr failing: 502, like the outage.
        PinError::NoIdentifier => AppError::of(
            ErrorCode::Upstream,
            "plex.tv created a sign-in this build cannot follow.",
        ),
        // `PinError` is `#[non_exhaustive]`; a transport failure and anything
        // added later read the same way to an operator, and neither is
        // something they can correct from here.
        PinError::Transport(_) | _ => AppError::of(
            ErrorCode::Upstream,
            "plex.tv did not respond. Sign-in cannot continue until it does.",
        ),
    }
    .caused_by(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_body_rejects_a_field_it_does_not_know() {
        let error = serde_json::from_str::<StartPin>(r#"{"oauth":true,"admin":true}"#)
            .expect_err("an unknown field must be refused");
        assert!(error.to_string().contains("admin"), "{error}");
    }

    #[test]
    fn a_transport_failure_answers_the_status_the_route_documents() {
        // 502, not 500: the route's contract declares an upstream failure, and
        // a client that received 500 could not tell an outage from a fault.
        let error = plex_failure(PinError::NoIdentifier);
        assert_eq!(error.problem().code, ErrorCode::Upstream);
        assert_eq!(
            error.problem().code.status(),
            axum::http::StatusCode::BAD_GATEWAY
        );
        assert!(
            !error.problem().message.contains("identifier this build"),
            "{}",
            error.problem().message
        );
    }

    fn host(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if !value.is_empty() {
            headers.insert(
                HOST,
                axum::http::HeaderValue::from_str(value).expect("a valid header"),
            );
        }
        headers
    }

    #[test]
    fn a_return_to_this_instance_is_allowed() {
        assert!(returns_here(
            "http://afisharr.example/login",
            Scheme::Http,
            &host("afisharr.example"),
        ));
        assert!(returns_here(
            "https://afisharr.example:8484/login?x=1",
            Scheme::Https,
            &host("afisharr.example:8484"),
        ));
    }

    #[test]
    fn a_return_to_somebody_else_is_refused() {
        // The hole this closes: the caller posts `forwardUrl`, the endpoint
        // embeds it in a genuine `app.plex.tv/auth` URL, and whoever finishes
        // the sign-in lands on the attacker's page.
        for target in [
            "https://evil.example",
            "https://evil.example/afisharr.example",
            "https://afisharr.example.evil.example/login",
            "https://afisharr.example@evil.example/login",
            "//evil.example/login",
            "javascript:alert(1)",
            "data:text/html,<script></script>",
            "not a url",
        ] {
            assert!(
                !returns_here(target, Scheme::Https, &host("afisharr.example")),
                "{target} must not be treated as this instance"
            );
        }
    }

    #[test]
    fn a_default_port_and_its_spelling_are_one_instance() {
        assert!(returns_here(
            "https://afisharr.example:443/login",
            Scheme::Https,
            &host("afisharr.example"),
        ));
    }

    #[test]
    fn the_scheme_the_request_arrived_over_is_part_of_the_comparison() {
        // A plaintext hop must not mint a return to an `https` origin it cannot
        // prove it is, and vice versa: the scheme comes from the resolved
        // client context, which is where a forwarded claim is judged.
        assert!(!returns_here(
            "https://afisharr.example/login",
            Scheme::Http,
            &host("afisharr.example"),
        ));
        assert!(!returns_here(
            "http://afisharr.example/login",
            Scheme::Https,
            &host("afisharr.example"),
        ));
    }

    #[test]
    fn a_request_with_no_host_to_compare_against_proves_nothing() {
        assert!(!returns_here(
            "https://afisharr.example/login",
            Scheme::Https,
            &host(""),
        ));
    }
}
