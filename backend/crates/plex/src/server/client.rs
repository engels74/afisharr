// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The one place a request to a Plex Media Server is built and sent.

use afisharr_sources::outbound::{
    Deadline, HeaderName, HeaderValue, Method, OutboundClient, OutboundError, RequestBody, Response,
};
use serde::de::DeserializeOwned;
use url::Url;

use crate::{
    identity::{ClientIdentity, PLEX_TOKEN},
    server::{Container, ServerAddress, ServerError, ServerToken, redact_credentials},
};

/// How long a call the connectivity check waits on may take.
///
/// Far shorter than the client default, because this is what the Settings page
/// renders a live connection state from: an operator waiting thirty seconds for
/// "unreachable" has been told nothing they did not already suspect, and the
/// server is on their own network. One constant for every call the check makes,
/// so the page's worst case stays a multiple of a single number rather than the
/// sum of two that drifted apart.
pub(crate) const CHECK_DEADLINE: Deadline = Deadline::of(std::time::Duration::from_secs(5));

/// A client bound to one Plex Media Server.
///
/// Cloning is cheap: the transport underneath is an `Arc`, and everything else
/// here is a handful of pre-validated header values.
#[derive(Debug, Clone)]
pub struct PlexServerClient {
    outbound: OutboundClient,
    identity: ClientIdentity,
    address: ServerAddress,
    token: ServerToken,
}

impl PlexServerClient {
    /// A client that reaches `address` as `identity`, presenting `token`.
    #[must_use]
    pub fn new(
        outbound: OutboundClient,
        identity: ClientIdentity,
        address: ServerAddress,
        token: ServerToken,
    ) -> Self {
        Self {
            outbound,
            identity,
            address,
            token,
        }
    }

    /// Where this client is pointed.
    #[must_use]
    pub const fn address(&self) -> &ServerAddress {
        &self.address
    }

    /// The `X-Plex-*` identity every request from this client carries.
    ///
    /// Named for the headers rather than for the server, because
    /// [`PlexServerClient::identity`] asks the server who *it* is, and two
    /// methods called `identity` on one type would be two different questions
    /// wearing one name.
    #[must_use]
    pub const fn client_identity(&self) -> &ClientIdentity {
        &self.identity
    }

    /// Reads one endpoint's body without interpreting it.
    ///
    /// The one caller is the release-lane contract test, and it needs the raw
    /// envelope rather than a parsed value: a comparison over parsed values
    /// would only ever see the fields this build already reads, which is the
    /// half of the response contract that cannot drift without a compile
    /// error. What drifts silently is everything else in the answer.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer or
    /// refused.
    pub async fn raw_get(&self, url: &Url) -> Result<Response, ServerError> {
        self.send(Method::GET, url, None, &[]).await
    }

    /// Builds an endpoint URL against this server's address.
    pub(crate) fn endpoint(
        &self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<Url, ServerError> {
        self.address
            .endpoint(path, query)
            .map_err(|source| self.unusable_path(path, source))
    }

    /// Builds an endpoint URL from a `key` the *server* supplied.
    ///
    /// See [`ServerAddress::discovered_endpoint`] for what this refuses and
    /// why. The refusal is a failure rather than a skipped call, because the
    /// caller asked for the choices of one filter and there are none to report
    /// — an empty list here would be an empty vocabulary presented as a fact
    /// the server stated (P1).
    ///
    /// # Errors
    /// Returns [`ServerError::ForeignEndpoint`] when the key does not land on
    /// this server, and [`ServerError::Address`] when it is not a reference
    /// this build can resolve at all.
    pub(crate) fn discovered_endpoint(&self, key: &str) -> Result<Url, ServerError> {
        self.address
            .discovered_endpoint(key)
            .map_err(|source| self.unusable_path(key, source))?
            .ok_or_else(|| ServerError::ForeignEndpoint {
                host: self.address.host().to_owned(),
                // Redacted for the reason every other rendering of an address
                // is: this string reaches the page and the logs, and a key that
                // arrived as a whole URL can carry userinfo.
                key: redact_credentials(key),
            })
    }

    /// Wraps a path that is not a reference this build can resolve.
    fn unusable_path(&self, path: &str, source: url::ParseError) -> ServerError {
        ServerError::Address {
            host: self.address.host().to_owned(),
            path: redact_credentials(path),
            source,
        }
    }

    /// Sends one request at the client's own deadline.
    pub(crate) async fn send(
        &self,
        method: Method,
        url: &Url,
        body: Option<RequestBody>,
        extra: &[(HeaderName, HeaderValue)],
    ) -> Result<Response, ServerError> {
        self.send_within(method, url, body, extra, self.outbound.deadline())
            .await
    }

    /// Sends one request at a deadline no longer than the client's.
    ///
    /// The connectivity check shortens it: an operator watching a settings page
    /// learns nothing from thirty seconds of nothing, and the machine-identifier
    /// call is one round trip against a server on their own network.
    pub(crate) async fn send_within(
        &self,
        method: Method,
        url: &Url,
        body: Option<RequestBody>,
        extra: &[(HeaderName, HeaderValue)],
        deadline: Deadline,
    ) -> Result<Response, ServerError> {
        let mut headers = self.identity.headers();
        headers.push((PLEX_TOKEN, self.token.header_value()));
        headers.extend_from_slice(extra);

        self.outbound
            .send(method, url, &headers, body, deadline)
            .await
            .map_err(|source| self.transport(source))
    }

    /// Sends one request and reads the `MediaContainer` it answers with.
    pub(crate) async fn container<T: DeserializeOwned>(
        &self,
        method: Method,
        url: &Url,
        body: Option<RequestBody>,
    ) -> Result<T, ServerError> {
        let response = self.send(method, url, body, &[]).await?;
        self.parse_container(&response)
    }

    /// Reads a `MediaContainer` out of an answer already in hand.
    pub(crate) fn parse_container<T: DeserializeOwned>(
        &self,
        response: &Response,
    ) -> Result<T, ServerError> {
        response
            .json::<Container<T>>(self.address.host())
            .map(|container| container.media_container)
            .map_err(|source| self.transport(source))
    }

    /// Wraps an outbound failure with the host it was against.
    fn transport(&self, source: OutboundError) -> ServerError {
        ServerError::Transport {
            host: self.address.host().to_owned(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn client_at(base: &str) -> PlexServerClient {
        PlexServerClient::new(
            OutboundClient::new("afisharr/test").expect("the transport must build"),
            ClientIdentity::new("01JABCDEF", "Living Room", "0.1.0").expect("a valid identity"),
            ServerAddress::parse(base).expect("a valid address"),
            ServerToken::new("plex-token").expect("a header-safe token"),
        )
    }

    #[test]
    fn an_endpoint_is_composed_against_the_configured_address() {
        let client = client_at("http://plex.lan:32400");
        let url = client
            .endpoint("library/sections", &[("type".to_owned(), "1".to_owned())])
            .expect("a valid endpoint");
        assert_eq!(
            url.as_str(),
            "http://plex.lan:32400/library/sections?type=1"
        );
    }

    #[test]
    fn a_container_is_unwrapped_from_an_answer() {
        #[derive(Debug, serde::Deserialize)]
        struct Body {
            size: u32,
        }

        let client = client_at("http://plex.lan:32400");
        let response = Response {
            status: 200,
            body: r#"{"MediaContainer":{"size":7}}"#.to_owned(),
        };
        let body: Body = client.parse_container(&response).expect("parses");
        assert_eq!(body.size, 7);
    }

    #[test]
    fn a_body_this_build_cannot_read_names_the_server_rather_than_the_body() {
        let client = client_at("http://plex.lan:32400");
        let response = Response {
            status: 200,
            body: "<html>not plex</html>".to_owned(),
        };
        let error = client
            .parse_container::<serde_json::Value>(&response)
            .expect_err("html is not a container");
        assert!(error.to_string().contains("plex.lan"), "{error}");
        assert!(error.server_answered());
    }
}
