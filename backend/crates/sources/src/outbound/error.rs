// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What can go wrong on an outbound request.

use thiserror::Error;

/// A failure reaching, or reading from, an external service.
///
/// [`OutboundError::Unreachable`] and [`OutboundError::Status`] are kept apart
/// because the difference is the whole of failure pattern P1: a service that
/// answered "nothing here" has told us something, and a service that did not
/// answer has told us nothing at all. Collapsing them into one "request
/// failed" is how an unreachable source becomes an empty collection.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OutboundError {
    /// The URL could not be built from the configured base and path.
    #[error("{host} could not be addressed")]
    Address {
        /// The host the request was for.
        host: String,
        /// The underlying failure.
        #[source]
        source: url::ParseError,
    },

    /// No answer arrived: the connection failed, or the deadline elapsed.
    #[error("{host} did not respond within {timeout_millis}ms")]
    Unreachable {
        /// The host the request was for.
        host: String,
        /// The deadline that was in force.
        timeout_millis: u64,
        /// The underlying failure.
        #[source]
        source: reqwest::Error,
    },

    /// An answer arrived, and it was a refusal.
    #[error("{host} answered {status}")]
    Status {
        /// The host that answered.
        host: String,
        /// The status it answered with.
        status: u16,
        /// The body, truncated, for the collapsed technical detail the
        /// interface offers (PRD §8.4).
        body: String,
    },

    /// An answer arrived and did not parse as what the adapter expected.
    #[error("{host} answered with a body this build cannot read")]
    Malformed {
        /// The host that answered.
        host: String,
        /// The underlying failure.
        #[source]
        source: serde_json::Error,
    },

    /// An answer arrived and its body was larger than this client will hold.
    ///
    /// Its own variant because it is neither of the two above: the service
    /// answered, so it is not unreachable, and nothing was refused, so it is not
    /// a status. What it says is that this instance stopped reading — the one
    /// fact an operator needs to tell a provider incident from a limit here.
    #[error("{host} answered with a body larger than {limit_bytes} bytes")]
    Oversized {
        /// The host that answered.
        host: String,
        /// The cap that was in force.
        limit_bytes: usize,
    },
}

impl OutboundError {
    /// The host this failure is about.
    #[must_use]
    pub fn host(&self) -> &str {
        match self {
            Self::Address { host, .. }
            | Self::Unreachable { host, .. }
            | Self::Status { host, .. }
            | Self::Malformed { host, .. }
            | Self::Oversized { host, .. } => host,
        }
    }

    /// Whether the service answered at all.
    ///
    /// The one question an adapter must ask before treating an empty result as
    /// a fact (`I-SRC-1`).
    #[must_use]
    pub const fn service_answered(&self) -> bool {
        matches!(
            self,
            Self::Status { .. } | Self::Malformed { .. } | Self::Oversized { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_counts_as_an_answer_and_a_timeout_does_not() {
        let refused = OutboundError::Status {
            host: "plex.tv".to_owned(),
            status: 404,
            body: String::new(),
        };
        assert!(refused.service_answered());
        assert_eq!(refused.host(), "plex.tv");
    }

    #[test]
    fn the_message_names_the_host_and_the_status() {
        let refused = OutboundError::Status {
            host: "plex.tv".to_owned(),
            status: 401,
            body: String::new(),
        };
        assert_eq!(refused.to_string(), "plex.tv answered 401");
    }
}
