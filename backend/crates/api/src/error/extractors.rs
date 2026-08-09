// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The request extractors, failing in this surface's one shape.
//!
//! Axum's own extractors reject a request before the handler runs and answer
//! with their own plain-text body. Those rejections cover the commonest request
//! failures there are — malformed JSON, a missing field, the wrong content
//! type, an absent query parameter — and a generated client that can narrow
//! every failure a handler produces but not the ones the extractor produces has
//! a hole in exactly the place a form posts into (`I-UX-2`, §24.5).

use axum::{
    Json,
    extract::{
        FromRequest, FromRequestParts, Query, Request,
        rejection::{JsonRejection, QueryRejection},
    },
    http::request::Parts,
};

use crate::error::{AppError, ErrorCode, Problem};

/// `Json`, failing in this surface's one shape.
///
/// A newtype rather than a `Json` alias: the rejection type is what carries the
/// mapping, and it can only be changed by owning the extractor.
#[derive(Debug)]
pub struct JsonBody<T>(
    /// The deserialised body.
    pub T,
);

impl<S, T> FromRequest<S> for JsonBody<T>
where
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(request, state)
            .await
            .map(|Json(body)| Self(body))
            .map_err(refusal)
    }
}

/// Renders an extractor rejection as a `Problem` the client can narrow.
///
/// The message is chosen here rather than taken from the rejection: serde's
/// text names Rust field paths and byte offsets, which is a description of this
/// build's types rather than of what the caller sent (D-029).
fn refusal(rejection: JsonRejection) -> AppError {
    let message = match &rejection {
        JsonRejection::MissingJsonContentType(_) => {
            "That request must be sent as application/json."
        }
        JsonRejection::JsonSyntaxError(_) => "That request body is not valid JSON.",
        JsonRejection::JsonDataError(_) => {
            "That request body is missing a required field, or carries one this endpoint \
             does not accept."
        }
        // `JsonRejection` is `#[non_exhaustive]`; a body that could not be read
        // and anything added later are the same fact to the caller.
        _ => "That request body could not be read.",
    };
    AppError::new(Problem::new(ErrorCode::Invalid, message)).caused_by(rejection)
}

/// `Query`, failing in this surface's one shape.
///
/// A parameter this route requires and did not get is a `Problem` like any
/// other refusal, not a plain-text 400 the client has to guess at.
#[derive(Debug)]
pub struct QueryParams<T>(
    /// The deserialised query string.
    pub T,
);

impl<S, T> FromRequestParts<S> for QueryParams<T>
where
    Query<T>: FromRequestParts<S, Rejection = QueryRejection>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(query)| Self(query))
            .map_err(|rejection| {
                AppError::new(Problem::new(
                    ErrorCode::Invalid,
                    "That request is missing a parameter, or carries one this endpoint \
                     does not accept.",
                ))
                .caused_by(rejection)
            })
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::header::CONTENT_TYPE};
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Payload {
        name: String,
    }

    fn request(content_type: Option<&str>, body: &str) -> Request {
        let mut builder = axum::http::Request::builder().method("POST").uri("/");
        if let Some(value) = content_type {
            builder = builder.header(CONTENT_TYPE, value);
        }
        builder.body(Body::from(body.to_owned())).expect("builds")
    }

    async fn refused(content_type: Option<&str>, body: &str) -> Problem {
        let error = JsonBody::<Payload>::from_request(request(content_type, body), &())
            .await
            .expect_err("the extractor must refuse");
        error.problem().clone()
    }

    #[tokio::test]
    async fn a_well_formed_body_is_accepted() {
        let JsonBody(body) = JsonBody::<Payload>::from_request(
            request(Some("application/json"), r#"{"name":"a"}"#),
            &(),
        )
        .await
        .expect("a well-formed body must be accepted");
        assert_eq!(body.name, "a");
    }

    #[tokio::test]
    async fn malformed_json_is_the_one_shape_and_not_axums_own_text() {
        let problem = refused(Some("application/json"), "{").await;
        assert_eq!(problem.code, ErrorCode::Invalid);
        assert!(problem.message.contains("valid JSON"), "{problem:?}");
    }

    #[tokio::test]
    async fn a_missing_field_is_the_one_shape() {
        let problem = refused(Some("application/json"), "{}").await;
        assert_eq!(problem.code, ErrorCode::Invalid);
    }

    #[tokio::test]
    async fn the_wrong_content_type_is_the_one_shape() {
        let problem = refused(Some("text/plain"), r#"{"name":"a"}"#).await;
        assert_eq!(problem.code, ErrorCode::Invalid);
        assert!(problem.message.contains("application/json"), "{problem:?}");
    }

    #[tokio::test]
    async fn no_refusal_names_a_rust_type_or_a_byte_offset() {
        // The cause goes to the log; the body is what the operator reads.
        for (content_type, body) in [
            (Some("application/json"), "{"),
            (Some("application/json"), r#"{"other":1}"#),
            (None, r#"{"name":"a"}"#),
        ] {
            let problem = refused(content_type, body).await;
            assert!(!problem.message.contains("Payload"), "{problem:?}");
            assert!(!problem.message.contains("column"), "{problem:?}");
        }
    }

    #[tokio::test]
    async fn a_missing_query_parameter_is_the_one_shape_too() {
        #[derive(Debug, Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Params {
            #[allow(dead_code)]
            root: String,
        }

        let (mut parts, ()) = axum::http::Request::builder()
            .uri("/api/files?path=posters")
            .body(())
            .expect("builds")
            .into_parts();
        let error = QueryParams::<Params>::from_request_parts(&mut parts, &())
            .await
            .expect_err("a missing parameter must be refused");
        assert_eq!(error.problem().code, ErrorCode::Invalid);
        assert!(!error.problem().message.contains("root"), "{error:?}");
    }
}
