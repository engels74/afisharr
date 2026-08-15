// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `GET /` — whether the server still accepts this instance's token.
//!
//! Its own call because the identity call is not one. `GET /identity` answers
//! before authentication, so a server whose token was revoked last week names
//! itself exactly as it always did: a check built on that call alone reports a
//! working connection for a credential Plex no longer honours, which is the
//! commonest way a Plex integration breaks and the one state it could never
//! see. An operator told "reachable" for a dead token has been sent looking for
//! a problem that is not there (`I-UX-2`).
//!
//! The server root is the cheapest endpoint that does require the token, and
//! nothing is read out of it: what it would say about the server is already
//! known from the identity call, so only whether it answered matters here.

use afisharr_sources::outbound::Method;

use crate::server::{PlexServerClient, ServerError, client::CHECK_DEADLINE};

impl PlexServerClient {
    /// Asks the server whether it still accepts this client's token.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer, and
    /// the same variant carrying a `401` or `403` when it answered by refusing
    /// the token — the distinction a caller draws with
    /// [`ServerError::refused_status`], because "did not answer" and "refused
    /// what it was given" are opposite remedies.
    #[tracing::instrument(skip(self))]
    pub async fn verify_credential(&self) -> Result<(), ServerError> {
        // The server root is the configured address itself: `endpoint` joins a
        // server-relative path, and an empty one resolves to the base. An
        // operator whose Plex sits behind a proxy at `/pms` is asked about
        // `/pms/` rather than about the host root, which is somebody else's.
        let url = self.endpoint("", &[])?;
        self.send_within(Method::GET, &url, None, &[], CHECK_DEADLINE)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use afisharr_sources::outbound::{Deadline, OutboundClient};

    use crate::{
        identity::ClientIdentity,
        server::{PlexServerClient, ServerAddress, ServerToken, client::CHECK_DEADLINE},
    };

    fn client_at(base: &str) -> PlexServerClient {
        PlexServerClient::new(
            OutboundClient::new("afisharr/test").expect("the transport must build"),
            ClientIdentity::new("01JABCDEF", "Living Room", "0.1.0").expect("a valid identity"),
            ServerAddress::parse(base).expect("a valid address"),
            ServerToken::new("plex-token").expect("a header-safe token"),
        )
    }

    #[test]
    fn the_credential_call_asks_the_configured_root_and_not_the_hosts() {
        // The reverse-proxy case. A request to `/` on a host serving Plex under
        // `/pms` asks whatever else lives at that address, and whatever it
        // answers would be read here as a verdict on the Plex token.
        let url = client_at("https://home.example/pms")
            .endpoint("", &[])
            .expect("a valid endpoint");
        assert_eq!(url.as_str(), "https://home.example/pms/");
    }

    #[test]
    fn an_ordinary_address_resolves_to_the_server_root() {
        let url = client_at("http://plex.lan:32400")
            .endpoint("", &[])
            .expect("a valid endpoint");
        assert_eq!(url.as_str(), "http://plex.lan:32400/");
    }

    #[test]
    fn the_credential_call_is_bounded_the_way_the_identity_call_is() {
        // Both are round trips the Settings page waits on, and a page whose
        // worst case is one short deadline plus one long one is a page that
        // waits for the long one.
        assert!(CHECK_DEADLINE < Deadline::DEFAULT);
    }
}
