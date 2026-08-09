// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The closed set of failure codes, and the status each maps to.

use axum::http::StatusCode;
use serde::Serialize;
use utoipa::ToSchema;

/// What kind of failure this is.
///
/// A closed enum with an exhaustive status mapping, so the generated client
/// narrows on a value it knows rather than on a number. Adding a variant is a
/// contract change that shows up in the regenerated client, which is the
/// point: a new failure mode the interface has never heard of should not be
/// able to arrive silently as a 500.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    /// The request body or a parameter did not validate.
    Invalid,
    /// No credential was presented, or the one presented is not accepted.
    Unauthenticated,
    /// A credential was presented and does not permit this.
    Forbidden,
    /// The thing addressed does not exist.
    NotFound,
    /// The request conflicts with the current state.
    Conflict,
    /// A human decision is owed before this can proceed (PRD §8.1).
    Blocked,
    /// A rate limit was exceeded.
    RateLimited,
    /// The instance has not been claimed, or setup is not finished.
    SetupRequired,
    /// A service this instance depends on did not answer, or answered with
    /// something this build cannot use.
    ///
    /// Its own code rather than `Internal`: an operator whose plex.tv is
    /// unreachable and an operator who has found a fault in Afisharr need
    /// different things, and a client that cannot tell the two apart tells its
    /// user neither.
    Upstream,
    /// Something failed that the caller cannot correct.
    Internal,
}

impl ErrorCode {
    /// The HTTP status this code is reported under.
    #[must_use]
    pub const fn status(self) -> StatusCode {
        match self {
            Self::Invalid => StatusCode::BAD_REQUEST,
            Self::Unauthenticated => StatusCode::UNAUTHORIZED,
            Self::NotFound => StatusCode::NOT_FOUND,
            // 409 for both Conflict and Blocked: a blocked request has not
            // failed, it is waiting on a decision, and the client tells the two
            // apart by the code rather than by the number (PRD §7.14).
            Self::Conflict | Self::Blocked => StatusCode::CONFLICT,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            // 403 for both: an unclaimed instance is refusing a caller who has
            // not proved console access, which is a permission answer. The
            // client tells them apart by the code, as with Conflict above.
            Self::Forbidden | Self::SetupRequired => StatusCode::FORBIDDEN,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            // 502, and not 500: the fault is upstream of this instance, and a
            // 500 tells an operator to read a log that will say nothing.
            Self::Upstream => StatusCode::BAD_GATEWAY,
        }
    }

    /// Whether a failure of this kind should be logged at `error!`.
    ///
    /// A rejected password and a rate-limited caller are the surface working,
    /// not the surface failing. Logging them at error level trains an operator
    /// to ignore the level that matters. An upstream outage is the same: it is
    /// real, it is not this instance's fault, and it is not actionable here.
    #[must_use]
    pub const fn is_a_fault(self) -> bool {
        matches!(self, Self::Internal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_CODE: [ErrorCode; 10] = [
        ErrorCode::Invalid,
        ErrorCode::Unauthenticated,
        ErrorCode::Forbidden,
        ErrorCode::NotFound,
        ErrorCode::Conflict,
        ErrorCode::Blocked,
        ErrorCode::RateLimited,
        ErrorCode::SetupRequired,
        ErrorCode::Upstream,
        ErrorCode::Internal,
    ];

    #[test]
    fn every_code_maps_to_a_client_or_server_error_status() {
        for code in EVERY_CODE {
            let status = code.status();
            assert!(
                status.is_client_error() || status.is_server_error(),
                "{code:?} maps to {status}"
            );
        }
    }

    #[test]
    fn only_an_internal_failure_is_a_fault() {
        for code in EVERY_CODE {
            assert_eq!(
                code.is_a_fault(),
                code == ErrorCode::Internal,
                "{code:?} reported the wrong severity"
            );
        }
    }

    #[test]
    fn blocked_and_conflict_share_a_status_and_differ_on_the_wire() {
        assert_eq!(ErrorCode::Blocked.status(), ErrorCode::Conflict.status());
        assert_ne!(
            serde_json::to_string(&ErrorCode::Blocked).expect("serialises"),
            serde_json::to_string(&ErrorCode::Conflict).expect("serialises")
        );
    }

    #[test]
    fn codes_serialise_as_camel_case_names() {
        assert_eq!(
            serde_json::to_string(&ErrorCode::SetupRequired).expect("serialises"),
            "\"setupRequired\""
        );
    }

    #[test]
    fn an_upstream_failure_is_a_bad_gateway_and_not_an_internal_error() {
        // The status the plex.tv routes declare. A 500 here would be a status
        // those operations never documented, and a client that cannot tell an
        // outage from a fault.
        assert_eq!(ErrorCode::Upstream.status(), StatusCode::BAD_GATEWAY);
        assert_ne!(ErrorCode::Upstream.status(), ErrorCode::Internal.status());
        assert_eq!(
            serde_json::to_string(&ErrorCode::Upstream).expect("serialises"),
            "\"upstream\""
        );
    }
}
