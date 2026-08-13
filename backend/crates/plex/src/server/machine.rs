// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `GET /identity` — who this server says it is.
//!
//! Its own call, and the cheapest one this crate makes. `I-ID-5` requires a
//! changed machine identifier to be detectable without a library fetch: every
//! rating key, every adoption, and every placement position in the database is
//! scoped to one server, so the check has to run at the head of a pass and cost
//! one round trip. Discovering a server swap half-way through a library sync is
//! discovering it after the writes have started.

use afisharr_sources::outbound::{Deadline, Method};
use serde::Deserialize;

use crate::server::{PlexServerClient, ServerError};

/// A Plex server's machine identifier.
///
/// The identity everything Plex-bound hangs off, so it is a newtype rather than
/// a `String`: `I-ID-5` compares two of these, and a comparison between a
/// machine identifier and a section key or a rating key must not typecheck
/// (P4 — identity is never carried by a value that can be mistaken for another
/// kind of identifier).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MachineIdentifier(String);

impl MachineIdentifier {
    /// Wraps a value read back from `plex_server.machine_identifier`.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The identifier as text, for storage and for a message that names it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MachineIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a server says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerIdentity {
    /// The identifier every Plex-bound row is scoped to.
    pub machine_identifier: MachineIdentifier,
    /// The server's version, which invalidates the discovered field cache
    /// when it changes (PRD §19.8).
    pub version: String,
    /// The name the operator gave the server, when it reports one.
    ///
    /// `GET /identity` answers before authentication and omits it; the full
    /// root answers it once a token is accepted. `None` is therefore "not
    /// reported by this call", never "the server has no name" (P1).
    pub friendly_name: Option<String>,
    /// The platform it runs on, when reported.
    pub platform: Option<String>,
}

/// How long the identity call may take.
///
/// Far shorter than the client default, because this is what the Settings page
/// renders a live connection state from: an operator waiting thirty seconds for
/// "unreachable" has been told nothing they did not already suspect, and the
/// server is on their own network.
const IDENTITY_DEADLINE: Deadline = Deadline::of(std::time::Duration::from_secs(5));

/// `GET /identity` exactly as Plex answers it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdentityBody {
    #[serde(default)]
    machine_identifier: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    friendly_name: Option<String>,
    #[serde(default)]
    platform: Option<String>,
}

impl PlexServerClient {
    /// Asks the server who it is.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer, and
    /// [`ServerError::Incomplete`] when it answered without an identifier —
    /// which is not "the identifier is unchanged", and must never be reconciled
    /// as one (P1).
    #[tracing::instrument(skip(self))]
    pub async fn identity(&self) -> Result<ServerIdentity, ServerError> {
        let url = self.endpoint("identity", &[])?;
        let response = self
            .send_within(Method::GET, &url, None, &[], IDENTITY_DEADLINE)
            .await?;
        let body: IdentityBody = self.parse_container(&response)?;
        ServerIdentity::try_from(body)
    }
}

impl TryFrom<IdentityBody> for ServerIdentity {
    type Error = ServerError;

    fn try_from(body: IdentityBody) -> Result<Self, Self::Error> {
        // Both refusals rather than a substituted value. An empty identifier
        // compared against a stored one reads as "a different server" and
        // blocks everything; a defaulted one reads as "the same server" and
        // writes rating keys from server A onto server B. Neither is an
        // observation, so neither is a fact this call may produce.
        let machine_identifier = body
            .machine_identifier
            .filter(|value| !value.is_empty())
            .ok_or(ServerError::Incomplete {
                call: "GET /identity",
                missing: "a machine identifier",
            })?;
        let version =
            body.version
                .filter(|value| !value.is_empty())
                .ok_or(ServerError::Incomplete {
                    call: "GET /identity",
                    missing: "a version",
                })?;
        Ok(Self {
            machine_identifier: MachineIdentifier::new(machine_identifier),
            version,
            friendly_name: body.friendly_name.filter(|name| !name.is_empty()),
            platform: body.platform.filter(|value| !value.is_empty()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(json: &str) -> Result<ServerIdentity, ServerError> {
        let parsed: IdentityBody = serde_json::from_str(json).expect("parses");
        ServerIdentity::try_from(parsed)
    }

    #[test]
    fn an_identity_answer_reads_the_identifier_and_the_version() {
        let identity =
            body(r#"{"machineIdentifier":"abc123","version":"1.41.0.1234","platform":"Linux"}"#)
                .expect("a complete answer");
        assert_eq!(identity.machine_identifier.as_str(), "abc123");
        assert_eq!(identity.version, "1.41.0.1234");
        assert_eq!(identity.platform.as_deref(), Some("Linux"));
        // `GET /identity` does not carry it, and inventing one would put a name
        // nobody chose on the Settings page.
        assert_eq!(identity.friendly_name, None);
    }

    #[test]
    fn an_answer_with_no_identifier_is_a_failure_and_not_a_match() {
        // The `I-ID-5` failure both ways round: defaulted to the stored value
        // it reconciles a different server silently, and defaulted to empty it
        // blocks a server that is perfectly fine.
        let error = body(r#"{"version":"1.41.0"}"#).expect_err("no identifier, no answer");
        assert!(matches!(error, ServerError::Incomplete { .. }));
        assert!(error.server_answered());
    }

    #[test]
    fn an_empty_identifier_is_treated_as_absent_rather_than_as_a_value() {
        assert!(body(r#"{"machineIdentifier":"","version":"1.41.0"}"#).is_err());
    }

    #[test]
    fn an_answer_with_no_version_is_a_failure_too() {
        // The version drives discovered-field invalidation (PRD §19.8). An
        // absent one recorded as empty makes every later comparison report a
        // version change and rediscover the whole field vocabulary each pass.
        let error = body(r#"{"machineIdentifier":"abc123"}"#).expect_err("no version, no answer");
        assert!(error.to_string().contains("version"), "{error}");
    }

    #[test]
    fn two_identifiers_compare_as_values() {
        assert_eq!(MachineIdentifier::new("abc"), MachineIdentifier::new("abc"));
        assert_ne!(MachineIdentifier::new("abc"), MachineIdentifier::new("xyz"));
        assert_eq!(MachineIdentifier::new("abc").to_string(), "abc");
    }

    #[test]
    fn the_identity_call_is_bounded_well_inside_the_client_default() {
        // The Settings page renders a live state from this call. The default
        // deadline is a budget for a cold provider on a WAN, not for one round
        // trip to a server on the operator's own network.
        assert!(IDENTITY_DEADLINE < Deadline::DEFAULT);
    }

    #[test]
    fn a_field_a_later_plex_adds_does_not_break_the_parse() {
        let identity = body(r#"{"machineIdentifier":"a","version":"1","newThing":[1]}"#)
            .expect("a complete answer");
        assert_eq!(identity.machine_identifier.as_str(), "a");
    }
}
