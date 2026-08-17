// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

// Each test binary that includes this module uses a different part of it, and
// an integration-test helper module is compiled once per binary.
#![allow(dead_code)]

//! Pointing a client at a running fake, and asking it something raw.

use afisharr_plex::{
    fake::FakePlex,
    identity::ClientIdentity,
    libraries::SectionKey,
    server::{PlexServerClient, ServerAddress, ServerToken},
};
use afisharr_sources::outbound::{HeaderName, HeaderValue, Method, OutboundClient, OutboundError};

/// The token every test presents unless it is testing the token gate.
pub const TOKEN: &str = "test-plex-token";

/// A client pointed at `fake`, presenting the usual token.
pub fn client_for(fake: &FakePlex) -> PlexServerClient {
    client_with_token(fake, TOKEN)
}

/// A client pointed at `fake`, presenting a token of the caller's choosing.
pub fn client_with_token(fake: &FakePlex, token: &str) -> PlexServerClient {
    PlexServerClient::new(
        OutboundClient::new("afisharr/test").expect("the transport must build"),
        ClientIdentity::new("01JTESTCLIENT", "Test Instance", "0.1.0").expect("a valid identity"),
        ServerAddress::parse(fake.base_url()).expect("a valid address"),
        ServerToken::new(token).expect("a header-safe token"),
    )
}

/// The movie library every default scenario builds.
pub fn movies() -> SectionKey {
    SectionKey::new("1")
}

/// One answer, exactly as it came off the wire.
#[derive(Debug, Clone)]
pub struct Raw {
    /// The status.
    pub status: u16,
    /// The body, as text.
    pub body: String,
}

/// Asks `fake` for `path` with the headers given and nothing else.
///
/// Raw because content negotiation is the subject: the client under test always
/// asks for JSON, and what a real Plex answers to a request that asks for
/// nothing is the half nothing here could previously see.
///
/// # Panics
/// Panics when the request could not be built or the fake did not answer.
pub async fn ask(fake: &FakePlex, path: &str, headers: &[(&'static str, &str)]) -> Raw {
    let outbound = OutboundClient::new("afisharr/test").expect("the transport must build");
    let url = url::Url::parse(&format!("{}/{path}", fake.base_url())).expect("a valid url");
    let headers: Vec<(HeaderName, HeaderValue)> = headers
        .iter()
        .map(|(name, value)| {
            (
                HeaderName::from_static(name),
                HeaderValue::from_str(value).expect("a header-safe value"),
            )
        })
        .collect();
    match outbound
        .send(Method::GET, &url, &headers, None, outbound.deadline())
        .await
    {
        Ok(response) => Raw {
            status: response.status,
            body: response.body,
        },
        // A refusal is an answer, and half these tests are about which refusal.
        Err(OutboundError::Status { status, body, .. }) => Raw { status, body },
        Err(error) => panic!("{path} did not answer: {error}"),
    }
}

/// Asks `fake` for `path` presenting the usual token and nothing else.
pub async fn ask_as_a_reference_client(fake: &FakePlex, path: &str) -> Raw {
    ask(fake, path, &[("x-plex-token", TOKEN)]).await
}
