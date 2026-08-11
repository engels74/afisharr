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
use axum::{Json, extract::State};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    authentication::session,
    error::{AppError, AppResult, ErrorCode, JsonBody, Problem},
    proxy::{ClientContext, PublicOrigin},
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
    /// It must be on this instance's configured `publicOrigin`. plex.tv
    /// redirects to whatever this asks for, so a target anywhere else turns the
    /// endpoint into a redirector wearing a legitimate `app.plex.tv/auth` URL,
    /// and the operator who completes the sign-in lands on somebody else's
    /// page.
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
        (status = 400, description = "The request body was not readable, or the return target is not this instance's configured origin", body = Problem),
        (status = 403, description = "Setup has not been completed on this instance", body = Problem),
        (status = 429, description = "Too many attempts", body = Problem),
        (status = 502, description = "plex.tv could not be reached", body = Problem),
    ),
)]
pub async fn start_plex_pin(
    State(state): State<ApiState>,
    client: ClientContext,
    jar: CookieJar,
    JsonBody(request): JsonBody<StartPin>,
) -> AppResult<(CookieJar, Json<PinStarted>)> {
    let mode = if request.oauth {
        Mode::OAuth
    } else {
        Mode::Pin
    };

    // Judged before plex.tv is called and before a row is stored: a return
    // target this instance will not stand behind is a bad request, not a
    // reason to spend an upstream call and leave an attempt behind.
    //
    // And judged before the limiter, for the same reason. `Bucket::Provider`
    // protects the operator's plex.tv quota; a request refused here never
    // reaches plex.tv, so counting it spends an allowance nothing was drawn
    // from. On a default install — where `publicOrigin` is unset and every
    // hosted sign-in is refused — that is 60 refusals a minute exhausting the
    // budget the *code* sign-in also draws on, and taking the working variant
    // down with the one that cannot work.
    //
    // Judged against the configured origin and against nothing in the request.
    // `Host` is written by whoever is calling, so an instance that compared
    // against it would accept `Host: evil.example` beside
    // `forwardUrl: https://evil.example/...` and hand the caller a genuine
    // `app.plex.tv/auth` URL that returns the operator to somebody else's page
    // (`I-SEC-1`).
    let forward_to = match (mode, request.forward_url.as_deref()) {
        (Mode::OAuth, Some(forward_to))
            if state
                .public_origin()
                .is_some_and(|origin| origin.covers(forward_to)) =>
        {
            Some(forward_to)
        }
        (Mode::OAuth, Some(_)) => {
            return Err(AppError::new(
                Problem::new(ErrorCode::Invalid, unreturnable(state.public_origin()))
                    .at("/forwardUrl"),
            ));
        }
        _ => None,
    };

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

/// Why a return target was refused, in terms the operator can act on.
///
/// Two different problems wearing one status. With no `publicOrigin` set there
/// is nothing to check a target against, and the answer has to name the setting
/// rather than blame the target — otherwise a first-run operator reads "only to
/// this instance" about the address they are sitting on.
fn unreturnable(configured: Option<&PublicOrigin>) -> String {
    match configured {
        Some(origin) => format!(
            "A Plex sign-in can only return to this instance, at {}.",
            origin.as_str()
        ),
        None => "This instance has no public origin configured, so a hosted Plex sign-in \
                 has no return address it can prove. Set http.publicOrigin, or sign in \
                 with a code."
            .to_owned(),
    }
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

    #[test]
    fn a_refusal_with_no_configured_origin_names_the_setting() {
        // The operator's two problems are different: nothing is configured, or
        // the target is not this instance. An answer that gave the second
        // sentence for the first case would send a first-run operator looking
        // for a fault in the address they are sitting on.
        let message = unreturnable(None);
        assert!(message.contains("publicOrigin"), "{message}");
        assert!(message.contains("code"), "{message}");
    }

    #[test]
    fn a_refusal_against_a_configured_origin_names_the_origin() {
        let origin = PublicOrigin::parse("https://afisharr.example").expect("a valid origin");
        let message = unreturnable(Some(&origin));
        assert!(message.contains("afisharr.example"), "{message}");
    }
}
