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
