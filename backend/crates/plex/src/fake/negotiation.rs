// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Which rendering a request asked for, and how an answer is sent back.
//!
//! A Plex Media Server answers XML unless the request asks for JSON. That is
//! not a detail: the reference client this phase is corrected against sends no
//! `Accept` header at all (`plexapi/config.py:53-68`) and parses every answer
//! as XML (`plexapi/server.py:759`), while Afisharr's own client asks for JSON
//! on every request. A fake that answered JSON to both is a fake only one of
//! the two readers can check.

use std::convert::Infallible;

use axum::{
    extract::FromRequestParts,
    http::{
        HeaderMap, StatusCode,
        header::{ACCEPT, CONTENT_TYPE},
        request::Parts,
    },
    response::{IntoResponse, Response},
};

use crate::fake::{element::Element, json, xml};

/// Which of the two renderings a request asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rendering {
    /// The default, and what a real server sends when nothing asks otherwise.
    Xml,
    /// What a request carrying `Accept: application/json` gets.
    Json,
}

impl Rendering {
    /// Reads the rendering one `Accept` header value asks for.
    ///
    /// A substring match rather than a parsed media-type list, deliberately:
    /// `Accept: application/json, text/plain, */*` is what a real client sends,
    /// and the question here is only whether JSON was asked for at all.
    fn of(accept: Option<&str>) -> Self {
        match accept {
            Some(value) if value.contains("application/json") => Self::Json,
            _ => Self::Xml,
        }
    }

    /// Reads the rendering a request's headers ask for.
    ///
    /// For the middleware, which answers before any extractor has run.
    pub(crate) fn of_headers(headers: &HeaderMap) -> Self {
        Self::of(headers.get(ACCEPT).and_then(|value| value.to_str().ok()))
    }

    /// An answer carrying `body`, at 200.
    pub(crate) const fn answer(self, body: Element) -> Answer {
        Answer {
            rendering: self,
            status: StatusCode::OK,
            body,
        }
    }

    /// A refusal, in the shape a real server refuses in.
    ///
    /// Plex answers a refusal inside the same envelope as everything else, so
    /// a client that parses the body of a `401` reads a status and a code
    /// rather than a parse error on top of the refusal it was already handling.
    pub(crate) fn refusal(self, status: StatusCode, code: u16, reason: &'static str) -> Response {
        Answer {
            rendering: self,
            status,
            body: Element::named("Response")
                .number("code", i64::from(code))
                .text("status", reason),
        }
        .into_response()
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Rendering {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self::of_headers(&parts.headers))
    }
}

/// One answer, in the rendering its request asked for.
#[derive(Debug)]
pub(crate) struct Answer {
    rendering: Rendering,
    status: StatusCode,
    body: Element,
}

impl IntoResponse for Answer {
    fn into_response(self) -> Response {
        match self.rendering {
            Rendering::Json => (
                self.status,
                [(CONTENT_TYPE, "application/json")],
                json::document(&self.body).to_string(),
            )
                .into_response(),
            Rendering::Xml => (
                self.status,
                [(CONTENT_TYPE, "text/xml;charset=utf-8")],
                xml::document(&self.body),
            )
                .into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_that_asks_for_nothing_gets_xml() {
        // The reference client sends no `Accept` header at all, and a real Plex
        // answers it in XML.
        assert_eq!(Rendering::of(None), Rendering::Xml);
    }

    #[test]
    fn a_request_that_asks_for_json_gets_json() {
        assert_eq!(Rendering::of(Some("application/json")), Rendering::Json);
        assert_eq!(
            Rendering::of(Some("application/json, text/plain, */*")),
            Rendering::Json
        );
    }

    #[test]
    fn a_request_that_asks_for_something_else_gets_xml_rather_than_a_refusal() {
        // A real server does not negotiate itself into a 406 here, and a fake
        // that did would fail a client for a reason no Plex produces.
        assert_eq!(Rendering::of(Some("text/html")), Rendering::Xml);
        assert_eq!(Rendering::of(Some("*/*")), Rendering::Xml);
    }

    #[tokio::test]
    async fn each_rendering_is_sent_under_its_own_content_type() {
        let json = Rendering::Json
            .answer(Element::named("MediaContainer"))
            .into_response();
        assert_eq!(
            json.headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );

        let xml = Rendering::Xml
            .answer(Element::named("MediaContainer"))
            .into_response();
        assert_eq!(
            xml.headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/xml;charset=utf-8")
        );
    }

    #[tokio::test]
    async fn a_refusal_carries_the_status_it_refused_with() {
        let refused = Rendering::Xml.refusal(StatusCode::UNAUTHORIZED, 1001, "Unauthorized");
        assert_eq!(refused.status(), StatusCode::UNAUTHORIZED);
    }
}
