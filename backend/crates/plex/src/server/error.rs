// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What can go wrong talking to a Plex Media Server.

use afisharr_sources::outbound::OutboundError;
use thiserror::Error;

/// A failure on one call to a Plex server.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ServerError {
    /// The server could not be reached, or refused.
    #[error("the Plex server at {host} could not be reached")]
    Transport {
        /// The host the request was for.
        host: String,
        /// The underlying failure.
        #[source]
        source: OutboundError,
    },

    /// A URL this crate composed is not a URL.
    #[error("{path} is not a path this build can request from {host}")]
    Address {
        /// The host the request was for.
        host: String,
        /// The path that could not be composed.
        path: String,
        /// The underlying failure.
        #[source]
        source: url::ParseError,
    },

    /// The server offered an endpoint that is not on the server.
    ///
    /// Its own variant rather than an [`ServerError::Address`], because nothing
    /// failed to compose: the key resolved perfectly, and it resolved somewhere
    /// else. The request is refused instead of sent, because it would have
    /// carried this instance's token to whatever host the answer named.
    #[error("the Plex server at {host} offered '{key}', which is not on that server")]
    ForeignEndpoint {
        /// The host the client is bound to.
        host: String,
        /// The key the answer carried, with any credential in it redacted.
        key: String,
    },

    /// The server answered, and the answer omitted something this build needs.
    ///
    /// Its own variant rather than a `Malformed` transport failure, because the
    /// two call for different moves: a body that did not parse is a version
    /// mismatch to report, and a body that parsed without the field the call
    /// exists to read is a call this build cannot complete against this server.
    #[error("the Plex server answered {call} without {missing}")]
    Incomplete {
        /// Which call was made.
        call: &'static str,
        /// What the answer did not carry.
        missing: &'static str,
    },
}

impl ServerError {
    /// Whether the server answered at all.
    ///
    /// The question every caller must ask before treating an empty result as a
    /// fact (`I-SRC-1`, P1). An answer this build could not use is still an
    /// answer; a connection that timed out is not.
    #[must_use]
    pub fn server_answered(&self) -> bool {
        match self {
            Self::Transport { source, .. } => source.service_answered(),
            Self::Incomplete { .. } => true,
            // No request was made, so nothing was observed. The key came out of
            // an earlier answer, but the call this failure belongs to never
            // reached the server — and a caller that read this as "answered"
            // would report an empty result as a fact about the library (P1).
            Self::Address { .. } | Self::ForeignEndpoint { .. } => false,
        }
    }

    /// The status the server refused with, when it refused.
    ///
    /// A 401 means the token is no longer accepted and a 404 means the thing
    /// addressed is gone; both are facts a caller acts on differently from an
    /// outage, and neither is discoverable from the message.
    #[must_use]
    pub fn refused_status(&self) -> Option<u16> {
        match self {
            Self::Transport {
                source: OutboundError::Status { status, .. },
                ..
            } => Some(*status),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refusal(status: u16) -> ServerError {
        ServerError::Transport {
            host: "plex.lan".to_owned(),
            source: OutboundError::Status {
                host: "plex.lan".to_owned(),
                status,
                body: String::new(),
            },
        }
    }

    #[test]
    fn a_refusal_is_an_answer_and_carries_its_status() {
        let error = refusal(401);
        assert!(error.server_answered());
        assert_eq!(error.refused_status(), Some(401));
    }

    #[test]
    fn an_answer_missing_a_field_is_still_an_answer() {
        let error = ServerError::Incomplete {
            call: "GET /identity",
            missing: "a machine identifier",
        };
        assert!(error.server_answered());
        assert_eq!(error.refused_status(), None);
        assert!(error.to_string().contains("machine identifier"), "{error}");
    }

    #[test]
    fn an_endpoint_that_is_not_on_the_server_is_not_an_answer_either() {
        // The request was refused before it was sent, so nothing was observed.
        // A caller reading this as "answered" would report the empty result as
        // a fact about the server's vocabulary (P1).
        let error = ServerError::ForeignEndpoint {
            host: "plex.lan".to_owned(),
            key: "http://collector.example/x".to_owned(),
        };
        assert!(!error.server_answered());
        assert_eq!(error.refused_status(), None);
        assert!(error.to_string().contains("collector.example"), "{error}");
        assert!(error.to_string().contains("plex.lan"), "{error}");
    }

    #[test]
    fn a_url_this_build_could_not_compose_is_not_an_answer() {
        let error = ServerError::Address {
            host: "plex.lan".to_owned(),
            path: "library/sections".to_owned(),
            source: url::ParseError::EmptyHost,
        };
        assert!(!error.server_answered());
    }
}
