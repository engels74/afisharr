// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What can go wrong exchanging a pin for a token.

use afisharr_sources::outbound::OutboundError;
use thiserror::Error;

/// A failure creating or polling a pin.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PinError {
    /// plex.tv could not be reached, or refused.
    #[error("plex.tv could not be reached")]
    Transport(#[from] OutboundError),

    /// plex.tv answered without the identifier the pin is polled by.
    #[error("plex.tv created a pin without an identifier this build can poll")]
    NoIdentifier,

    /// The pin was created under a different client identifier.
    ///
    /// Its own variant, and reported rather than tolerated: a token issued
    /// against a mismatched identifier is accepted by plex.tv and then refused
    /// by every subsequent call, which looks like an intermittent Plex outage
    /// for as long as nobody checks this. Failing here makes it visible at the
    /// moment it is caused (PRD §19.6).
    #[error(
        "plex.tv issued the pin under client identifier '{found}', not this instance's '{expected}'"
    )]
    ClientIdentifierMismatch {
        /// The identifier this instance sent.
        expected: String,
        /// The identifier plex.tv recorded.
        found: String,
    },
}

impl PinError {
    /// The status plex.tv answered with, when it answered at all.
    ///
    /// [`Self::Transport`] carries both halves of "the call did not work", and
    /// they are different facts to whoever is signing in: no answer means an
    /// outage and the only move is to wait, while a refusal means plex.tv is up
    /// and the pin the operator is holding may still be good on the next poll.
    /// Reported as one, a 429 or a 503 on the account lookup was rendered to the
    /// operator as "plex.tv did not respond" and threw away a live attempt.
    ///
    /// Answered here rather than matched at the call site so the HTTP surface
    /// need not depend on the outbound crate to tell the two apart.
    #[must_use]
    pub fn refused_status(&self) -> Option<u16> {
        match self {
            Self::Transport(OutboundError::Status { status, .. }) => Some(*status),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answered_refusal_is_told_apart_from_an_outage() {
        let refused = PinError::Transport(OutboundError::Status {
            host: "plex.tv".to_owned(),
            status: 429,
            body: String::new(),
        });
        assert_eq!(refused.refused_status(), Some(429));

        // No answer arrived, and nothing else here is a status either.
        assert_eq!(PinError::NoIdentifier.refused_status(), None);
    }
}
