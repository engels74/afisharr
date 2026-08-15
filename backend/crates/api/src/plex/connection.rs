// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the interface is told about the Plex connection.

use serde::Serialize;
use utoipa::ToSchema;

/// Where the Plex connection stands.
///
/// A closed enum the client narrows on, never a status code it infers from
/// (`I-UX-2`). Six variants and not three, because "nothing is configured",
/// "the server did not answer", and "the server refused what it was given" are
/// different problems with different remedies, and an operator shown the wrong
/// one goes looking for a network fault that is not there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum PlexConnectionState {
    /// No server is bound to this installation yet.
    NotConfigured,
    /// A server is bound, and no Plex credential is stored to reach it with.
    NoCredential,
    /// The bound server answered, and refused the credential it was given.
    ///
    /// Its own state and not [`Self::Unreachable`], because the two send the
    /// operator in opposite directions: a server that did not answer is a
    /// network fault to chase, and a server that answered `401` is a sign-in to
    /// repeat. Not [`Self::NoCredential`] either — a credential is stored, and
    /// being refused is the whole fact.
    CredentialRefused,
    /// The bound server answered, and it is the bound server.
    Reachable,
    /// The bound server did not answer, or answered something unusable.
    Unreachable,
    /// A *different* server answered at the bound address.
    ///
    /// Blocking (`I-ID-5`). Every rating key, adoption, and placement position
    /// in this database means something else on that server, so nothing is
    /// rebound and nothing is written until the operator decides.
    WrongServer,
}

impl PlexConnectionState {
    /// Whether this state blocks every Plex-bound action.
    #[must_use]
    pub const fn blocks(self) -> bool {
        matches!(self, Self::WrongServer)
    }
}

/// The whole answer the Settings page renders from.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlexConnection {
    /// Where the connection stands.
    pub state: PlexConnectionState,
    /// The address the bound server is reached at, when one is bound.
    pub base_url: Option<String>,
    /// The identifier this installation is bound to, when one is bound.
    ///
    /// Present on every state that has a binding, including
    /// [`PlexConnectionState::WrongServer`] — the operator's decision needs
    /// both sides of the mismatch, and an answer naming only the stranger tells
    /// them nothing about what they are being asked to abandon.
    pub bound_machine_identifier: Option<String>,
    /// The identifier that actually answered, when something did.
    pub observed_machine_identifier: Option<String>,
    /// The name the server reports, when it answered.
    pub friendly_name: Option<String>,
    /// The version it reports, when it answered.
    pub version: Option<String>,
    /// The technical detail behind an unreachable answer, for the collapsed
    /// detail §8.4 describes. Never a user-facing sentence: the interface
    /// composes those from its own catalogue (`I-UX-7`).
    pub detail: Option<String>,
    /// When this check ran, in epoch milliseconds.
    pub checked_at: i64,
}

impl PlexConnection {
    /// The answer for an installation with no server bound to it.
    #[must_use]
    pub const fn not_configured(checked_at: i64) -> Self {
        Self {
            state: PlexConnectionState::NotConfigured,
            base_url: None,
            bound_machine_identifier: None,
            observed_machine_identifier: None,
            friendly_name: None,
            version: None,
            detail: None,
            checked_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_STATE: [PlexConnectionState; 6] = [
        PlexConnectionState::NotConfigured,
        PlexConnectionState::NoCredential,
        PlexConnectionState::CredentialRefused,
        PlexConnectionState::Reachable,
        PlexConnectionState::Unreachable,
        PlexConnectionState::WrongServer,
    ];

    #[test]
    fn only_a_different_server_blocks() {
        for state in EVERY_STATE {
            assert_eq!(
                state.blocks(),
                state == PlexConnectionState::WrongServer,
                "{state:?}"
            );
        }
    }

    #[test]
    fn every_state_has_a_distinct_name_on_the_wire() {
        let mut names: Vec<String> = EVERY_STATE
            .into_iter()
            .map(|state| serde_json::to_string(&state).expect("serialises"))
            .collect();
        names.sort();
        let mut deduplicated = names.clone();
        deduplicated.dedup();
        assert_eq!(names, deduplicated, "two states share a name");
        assert!(names.contains(&"\"wrongServer\"".to_owned()), "{names:?}");
    }

    #[test]
    fn an_unconfigured_answer_claims_nothing_about_any_server() {
        let answer = PlexConnection::not_configured(1_700_000_000_000);
        let encoded = serde_json::to_value(&answer).expect("serialises");
        assert_eq!(encoded["state"], "notConfigured");
        assert!(encoded["baseUrl"].is_null());
        assert!(encoded["boundMachineIdentifier"].is_null());
        assert!(encoded["observedMachineIdentifier"].is_null());
    }
}
