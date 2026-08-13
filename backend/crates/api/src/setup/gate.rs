// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The two gates in front of the wizard.

use afisharr_core::setup::{CLAIM_COOKIE, ClaimState, RenewClaim, inspect};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::CookieJar;

use crate::{
    error::{AppError, AppResult, ErrorCode, Problem},
    proxy::ClientContext,
    security::set,
    state::ApiState,
};

/// Refuses every wizard endpoint once setup has finished.
///
/// 404 rather than 403: after `instance.setup_completed_at` is set, the setup
/// endpoints do not exist (PRD §19.6.1). A 403 would say "there is a wizard
/// here and you may not have it", which is both untrue and an invitation.
///
/// # Errors
/// Returns a `notFound` problem once setup is complete.
pub async fn require_setup_incomplete(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> AppResult<Response> {
    if state.setup_completed() {
        return Err(AppError::of(
            ErrorCode::NotFound,
            "Setup has already been completed on this instance.",
        ));
    }
    Ok(next.run(request).await)
}

/// Refuses any gated wizard endpoint without an active claim, and renews it
/// when there is one.
///
/// Renewal is not a separate mechanism (PRD §19.6.1): every gated request that
/// succeeds moves the lease's expiry ten minutes out and re-sets the cookie's
/// `Max-Age`, so an operator who keeps working never meets the timeout and one
/// who walks away releases the wizard without doing anything.
///
/// # Errors
/// Returns a `blocked` problem carrying the retry time when another browser
/// holds the claim, and a `setupRequired` problem when no claim is held at all.
pub async fn require_claim(
    State(state): State<ApiState>,
    client: ClientContext,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> AppResult<Response> {
    let now = state.clock().now();
    let cookie_value = jar
        .get(CLAIM_COOKIE)
        .map(|cookie| cookie.value().to_owned());

    let state_of_claim = inspect(state.database().readers(), cookie_value.as_deref(), now)
        .await
        .map_err(AppError::internal)?;

    let cookie_value = match state_of_claim {
        ClaimState::HeldByCaller { .. } => cookie_value.unwrap_or_default(),
        ClaimState::HeldByAnother { expires_at } => {
            return Err(blocked(now.millis_until(expires_at)));
        }
        ClaimState::Unclaimed => {
            return Err(AppError::of(
                ErrorCode::SetupRequired,
                "This instance is not claimed. Enter the token printed on the console.",
            ));
        }
    };

    let renewed = state
        .database()
        .writer()
        .submit(RenewClaim {
            cookie_value: cookie_value.clone(),
            at: now,
        })
        .await
        .map_err(AppError::internal)?;

    // The renewal races the lease expiring between the read and the write. If
    // it lost, the claim is gone and this request must not proceed on the
    // strength of a read that is now stale.
    let Some(expires_at) = renewed else {
        return Err(AppError::of(
            ErrorCode::SetupRequired,
            "The claim on this wizard has expired. Enter the token again.",
        ));
    };

    let mut response = next.run(request).await;

    // The handler gets the last word on this cookie. Completion answers with a
    // removal cookie of the same name and path, and a refreshed one appended
    // after it is the value the browser keeps — so the claim would survive a
    // setup that has explicitly expired it, which is the opposite of what the
    // completion contract says happens (PRD §19.6.1).
    if already_set(&response) {
        return Ok(response);
    }

    let refreshed = set(
        CLAIM_COOKIE,
        cookie_value,
        "/api/setup",
        now.millis_until(expires_at) / 1000,
        client.scheme,
        true,
    );
    if let Ok(header) = refreshed.to_string().parse() {
        response
            .headers_mut()
            .append(axum::http::header::SET_COOKIE, header);
    }
    Ok(response)
}

/// Whether the handler already spoke about the claim cookie.
fn already_set(response: &Response) -> bool {
    response
        .headers()
        .get_all(axum::http::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| {
            value
                .split(';')
                .next()
                .and_then(|pair| pair.split_once('='))
                .is_some_and(|(name, _)| name.trim() == CLAIM_COOKIE)
        })
}

/// The refusal a second browser gets, carrying when the hold lapses.
///
/// This is the one genuinely stranded case PRD §7.14 names: before an admin
/// exists, all three doors are shut, and the only correct answer is to say when
/// the wait ends.
fn blocked(remaining_millis: i64) -> AppError {
    let seconds = u64::try_from(remaining_millis.max(1000) / 1000).unwrap_or(1);
    AppError::new(
        Problem::new(
            ErrorCode::Blocked,
            "Another browser is holding the setup wizard.",
        )
        .retry_after(seconds),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blocked_answer_carries_the_retry_time_and_no_detail_about_the_holder() {
        let error = blocked(9 * 60 * 1000);
        assert_eq!(error.problem().code, ErrorCode::Blocked);
        assert_eq!(error.problem().retry_after_seconds, Some(9 * 60));
        assert!(!error.problem().message.contains("cookie"));
    }

    #[test]
    fn a_hold_that_lapses_within_a_second_still_reports_at_least_one() {
        assert_eq!(blocked(1).problem().retry_after_seconds, Some(1));
        assert_eq!(blocked(-5).problem().retry_after_seconds, Some(1));
    }

    #[test]
    fn a_response_that_already_removes_the_claim_is_left_alone() {
        // The completion handler expires the cookie. A refreshed one appended
        // after it is the value the browser keeps, so the claim would outlive
        // the setup that ended it.
        let removal =
            crate::security::expire(CLAIM_COOKIE, "/api/setup", crate::proxy::Scheme::Http);
        let response = Response::builder()
            .header(axum::http::header::SET_COOKIE, removal.to_string())
            .body(axum::body::Body::empty())
            .expect("the response must build");
        assert!(already_set(&response));
    }

    #[test]
    fn a_response_setting_some_other_cookie_still_gets_the_renewal() {
        let response = Response::builder()
            .header(axum::http::header::SET_COOKIE, "afisharr_csrf=abc; Path=/")
            .body(axum::body::Body::empty())
            .expect("the response must build");
        assert!(!already_set(&response));
    }
}
