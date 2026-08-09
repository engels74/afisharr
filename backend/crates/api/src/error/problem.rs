// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The failure shape every route answers with.

use serde::Serialize;
use utoipa::ToSchema;

use crate::error::ErrorCode;

/// What was expected against what arrived.
///
/// Carried as a pair rather than folded into the message, because the
/// interface renders them as two labelled values and a message it has to parse
/// is a message that stops being renderable the first time the wording changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Mismatch {
    /// What the field should have held, in the operator's terms.
    pub expected: String,
    /// What it held.
    pub actual: String,
}

/// The body of every failed response on this surface.
///
/// The JSON pointer is what lets a form put the message beside the field that
/// caused it, rather than in a banner at the top that names nothing. It is
/// RFC 6901 — `/sources/2/kind`, not `sources[2].kind` — so the client can walk
/// the request body it already holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// What kind of failure this is. The client narrows on this.
    pub code: ErrorCode,
    /// One sentence, in the operator's terms, naming what failed (PRD §8.4).
    pub message: String,
    /// An RFC 6901 pointer into the request body, when one field caused this.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pointer: Option<String>,
    /// The expected-versus-actual pair, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mismatch: Option<Mismatch>,
    /// When the caller may try again, in seconds. Set on a rate-limited or
    /// blocked answer, and matching the `Retry-After` header.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

impl Problem {
    /// A problem with a code and a message and nothing else.
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            pointer: None,
            mismatch: None,
            retry_after_seconds: None,
        }
    }

    /// The same problem, pointing at the field that caused it.
    #[must_use]
    pub fn at(mut self, pointer: impl Into<String>) -> Self {
        self.pointer = Some(pointer.into());
        self
    }

    /// The same problem, carrying what was expected and what arrived.
    #[must_use]
    pub fn expecting(mut self, expected: impl Into<String>, actual: impl Into<String>) -> Self {
        self.mismatch = Some(Mismatch {
            expected: expected.into(),
            actual: actual.into(),
        });
        self
    }

    /// The same problem, naming when the caller may try again.
    #[must_use]
    pub const fn retry_after(mut self, seconds: u64) -> Self {
        self.retry_after_seconds = Some(seconds);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_problem_omits_every_optional_field() {
        let encoded = serde_json::to_string(&Problem::new(ErrorCode::NotFound, "no such library"))
            .expect("serialises");
        assert_eq!(
            encoded,
            r#"{"code":"notFound","message":"no such library"}"#
        );
    }

    #[test]
    fn a_field_level_problem_carries_the_pointer_and_the_mismatch() {
        let problem = Problem::new(ErrorCode::Invalid, "that operator does not apply here")
            .at("/filters/0/operator")
            .expecting("one of: is, isNot", "greaterThan");
        let encoded = serde_json::to_value(&problem).expect("serialises");
        assert_eq!(encoded["pointer"], "/filters/0/operator");
        assert_eq!(encoded["mismatch"]["expected"], "one of: is, isNot");
        assert_eq!(encoded["mismatch"]["actual"], "greaterThan");
    }

    #[test]
    fn a_retry_time_is_carried_in_the_body_as_well_as_the_header() {
        let problem = Problem::new(ErrorCode::RateLimited, "too many attempts").retry_after(900);
        assert_eq!(problem.retry_after_seconds, Some(900));
    }

    #[test]
    fn the_pointer_is_a_json_pointer_rather_than_a_property_path() {
        // The client walks the body it already holds; a dotted path would need
        // parsing, and an index in brackets would need a second parser.
        let problem = Problem::new(ErrorCode::Invalid, "bad").at("/sources/2/kind");
        assert!(problem.pointer.is_some_and(|p| p.starts_with('/')));
    }
}
