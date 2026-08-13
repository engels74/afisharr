// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where a credentialed outbound call is allowed to end up.
//!
//! A separate test target because the fact under test is not in the answer the
//! caller receives: it is that a *second* host was never contacted. Proving
//! that takes two listeners and a recording between them, which is more
//! scaffolding than belongs beside the unit tests of one function.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use afisharr_sources::outbound::{
    Deadline, HeaderName, HeaderValue, Method, OutboundClient, OutboundError,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
use url::Url;

/// A host that answers one redirect, and the host it points at.
///
/// The second listener never writes a reply. It exists to record whether
/// anything reached it at all, which is the fact under test.
async fn a_server_that_redirects_elsewhere() -> (String, Arc<AtomicBool>, [JoinHandle<()>; 2]) {
    let elsewhere = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a port must be bindable");
    let target = elsewhere.local_addr().expect("the port must be readable");
    let reached = Arc::new(AtomicBool::new(false));
    let recording = Arc::clone(&reached);
    let listening = tokio::spawn(async move {
        if elsewhere.accept().await.is_ok() {
            recording.store(true, Ordering::SeqCst);
        }
    });

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a port must be bindable");
    let address = listener.local_addr().expect("the port must be readable");
    let redirecting = tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).await;
        let reply = format!(
            "HTTP/1.1 302 Found\r\nLocation: http://{target}/taken\r\nContent-Length: 0\r\n\r\n"
        );
        let _ = socket.write_all(reply.as_bytes()).await;
    });

    (
        format!("http://{address}/"),
        reached,
        [listening, redirecting],
    )
}

#[tokio::test]
async fn a_redirect_is_refused_rather_than_followed_with_the_credential_header() {
    // The theft this closes. The header below is how `PlexTvClient` sends the
    // operator's Plex token; reqwest's cross-origin scrub list is
    // `Authorization`, `Cookie`, `Proxy-Authorization`, and `WWW-Authenticate`,
    // and cannot know about a provider's own credential header. Under the
    // default policy the token arrived at whatever host the provider named,
    // and nothing in this process failed while it did.
    let (url, reached, servers) = a_server_that_redirects_elsewhere().await;
    let client = OutboundClient::new("afisharr/test").expect("the transport must build");

    let error = client
        .send(
            Method::GET,
            &Url::parse(&url).expect("a valid URL"),
            &[(
                HeaderName::from_static("x-plex-token"),
                HeaderValue::from_static("the-operator-token"),
            )],
            None,
            Deadline::DEFAULT,
        )
        .await
        .expect_err("a redirect is not a successful answer");

    assert!(
        matches!(error, OutboundError::Status { status: 302, .. }),
        "expected the redirect reported as a refusal, got {error}"
    );
    assert!(
        error.service_answered(),
        "the provider did answer, and an operator must be able to tell that from silence"
    );
    assert!(
        !reached.load(Ordering::SeqCst),
        "the redirect target must never be reached, and never with the token"
    );

    for server in servers {
        server.abort();
    }
}
