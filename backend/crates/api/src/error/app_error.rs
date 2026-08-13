// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The only thing a handler returns on the unhappy path.

use axum::{
    Json,
    http::{HeaderValue, header::RETRY_AFTER},
    response::{IntoResponse, Response},
};
use tracing::{error, warn};

use crate::error::{ErrorCode, Problem};

/// What every handler on this surface returns.
pub type AppResult<T> = Result<T, AppError>;

/// A failure, carrying the shape it will be rendered as.
///
/// The internal cause is held separately from the [`Problem`] and never
/// reaches the response. An operator gets a message they can act on; the chain
/// that produced it goes to the log, where it belongs (PRD §8.4).
#[derive(Debug)]
pub struct AppError {
    // Boxed so `Result<T, AppError>` stays cheap to move: this is the error
    // half of every handler's return type, and an inline `Problem` makes every
    // successful response pay for the shape of a failure.
    problem: Box<Problem>,
    cause: Option<String>,
}

impl AppError {
    /// A failure that renders as `problem`.
    #[must_use]
    pub fn new(problem: Problem) -> Self {
        Self {
            problem: Box::new(problem),
            cause: None,
        }
    }

    /// A failure with a code and a message.
    #[must_use]
    pub fn of(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::new(Problem::new(code, message))
    }

    /// The same failure, carrying the internal cause for the log.
    #[must_use]
    pub fn caused_by(mut self, cause: impl std::fmt::Display) -> Self {
        self.cause = Some(cause.to_string());
        self
    }

    /// An internal failure: one sentence for the operator, the chain for the log.
    ///
    /// The message is fixed rather than taken from the error, because an
    /// internal error's text names tables, paths, and drivers, and this
    /// instance may be facing the internet (D-029).
    #[must_use]
    pub fn internal(cause: impl std::fmt::Display) -> Self {
        Self::of(
            ErrorCode::Internal,
            "Afisharr could not complete that. The details are in the instance log.",
        )
        .caused_by(cause)
    }

    /// The wire shape this failure renders as.
    #[must_use]
    pub const fn problem(&self) -> &Problem {
        &self.problem
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.problem.code.status();

        if self.problem.code.is_a_fault() {
            error!(
                code = ?self.problem.code,
                cause = self.cause.as_deref().unwrap_or("unspecified"),
                "request failed"
            );
        } else {
            warn!(
                code = ?self.problem.code,
                cause = self.cause.as_deref().unwrap_or("unspecified"),
                "request refused"
            );
        }

        let retry_after = self.problem.retry_after_seconds;
        let mut response = (status, Json(*self.problem)).into_response();
        if let Some(seconds) = retry_after {
            // The header and the body carry the same number: a browser honours
            // the header, and the interface renders the body.
            if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
                response.headers_mut().insert(RETRY_AFTER, value);
            }
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;

    use super::*;

    async fn body_of(error: AppError) -> (axum::http::StatusCode, serde_json::Value) {
        let response = error.into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("the body must read");
        (
            status,
            serde_json::from_slice(&bytes).expect("the body must be JSON"),
        )
    }

    #[tokio::test]
    async fn a_failure_renders_as_the_one_shape() {
        let (status, body) = body_of(AppError::of(ErrorCode::NotFound, "no such library")).await;
        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "notFound");
        assert_eq!(body["message"], "no such library");
    }

    #[tokio::test]
    async fn an_internal_failure_does_not_leak_its_cause_into_the_body() {
        let (status, body) =
            body_of(AppError::internal("no such table: definitions (code 1)")).await;
        assert_eq!(status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            !body.to_string().contains("definitions"),
            "the cause must stay in the log: {body}"
        );
        assert_eq!(body["code"], "internal");
    }

    #[tokio::test]
    async fn a_rate_limited_failure_sets_retry_after_as_a_header_too() {
        let error = AppError::new(
            Problem::new(ErrorCode::RateLimited, "too many attempts").retry_after(900),
        );
        let response = error.into_response();
        assert_eq!(
            response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok()),
            Some("900")
        );
    }

    #[tokio::test]
    async fn a_blocked_failure_answers_409_and_says_so_in_the_body() {
        let (status, body) = body_of(AppError::of(ErrorCode::Blocked, "another browser")).await;
        assert_eq!(status, axum::http::StatusCode::CONFLICT);
        assert_eq!(body["code"], "blocked");
    }
}
