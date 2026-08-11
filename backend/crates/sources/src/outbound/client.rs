// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The client itself: one transport, timed, with a deadline it cannot lose.

use std::time::{Duration, Instant};

use reqwest::{
    Method,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use tracing::{info, warn};
use url::Url;

use crate::outbound::{Deadline, OutboundError};

/// A body that answered, with the status it answered under.
#[derive(Debug, Clone)]
pub struct Response {
    /// The HTTP status.
    pub status: u16,
    /// The body, as text.
    pub body: String,
}

impl Response {
    /// Parses the body as JSON.
    ///
    /// # Errors
    /// Returns [`OutboundError::Malformed`] when the body is not the shape the
    /// adapter declared, naming the host and nothing about the body's content.
    pub fn json<T: serde::de::DeserializeOwned>(&self, host: &str) -> Result<T, OutboundError> {
        serde_json::from_str(&self.body).map_err(|source| OutboundError::Malformed {
            host: host.to_owned(),
            source,
        })
    }
}

/// The single outbound HTTP client.
///
/// Cloning is cheap — `reqwest::Client` is an `Arc` internally — and is how a
/// crate gets its own handle without a second transport, a second connection
/// pool, or a second set of timings.
#[derive(Debug, Clone)]
pub struct OutboundClient {
    transport: reqwest::Client,
    deadline: Deadline,
}

impl OutboundClient {
    /// Builds the client at the default deadline.
    ///
    /// # Errors
    /// Returns [`OutboundError::Unreachable`] when the transport cannot be
    /// constructed, which on a supported platform means TLS initialisation
    /// failed and no outbound request will work.
    pub fn new(user_agent: &str) -> Result<Self, OutboundError> {
        Self::with_deadline(user_agent, Deadline::DEFAULT)
    }

    /// Builds the client at a chosen default deadline.
    ///
    /// # Errors
    /// As [`OutboundClient::new`].
    pub fn with_deadline(user_agent: &str, deadline: Deadline) -> Result<Self, OutboundError> {
        let transport = reqwest::Client::builder()
            .user_agent(user_agent)
            // Set here as well as per-request: `timeout` bounds the whole
            // request, and `connect_timeout` bounds the handshake, so a host
            // that accepts a socket and never negotiates fails fast instead of
            // spending the whole request budget.
            .timeout(deadline.as_duration())
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|source| OutboundError::Unreachable {
                host: "the outbound transport".to_owned(),
                timeout_millis: 0,
                source,
            })?;
        Ok(Self {
            transport,
            deadline,
        })
    }

    /// The deadline every request from this client carries.
    #[must_use]
    pub const fn deadline(&self) -> Deadline {
        self.deadline
    }

    /// Sends one request and reads its body.
    ///
    /// Timed on both paths: a request that fails at three seconds and one that
    /// succeeds at three seconds are equally interesting when a budget in
    /// PRD §21.2 is missed, and only one of them shows up in a success metric.
    ///
    /// # Errors
    /// Returns [`OutboundError::Unreachable`] when no answer arrived within the
    /// deadline, and [`OutboundError::Status`] when the answer was a refusal.
    #[tracing::instrument(skip(self, headers, body), fields(host = %url.host_str().unwrap_or("unknown")))]
    pub async fn send(
        &self,
        method: Method,
        url: &Url,
        headers: &[(HeaderName, HeaderValue)],
        body: Option<String>,
        deadline: Deadline,
    ) -> Result<Response, OutboundError> {
        let host = url.host_str().unwrap_or("unknown").to_owned();
        let effective = self.deadline.shortened_to(deadline.as_duration());

        let mut header_map = HeaderMap::with_capacity(headers.len());
        for (name, value) in headers {
            header_map.insert(name.clone(), value.clone());
        }

        let mut request = self
            .transport
            .request(method.clone(), url.clone())
            .headers(header_map)
            .timeout(effective.as_duration());
        if let Some(body) = body {
            request = request.body(body);
        }

        let timeout_millis = u64::try_from(effective.as_duration().as_millis()).unwrap_or(u64::MAX);
        let started = Instant::now();
        let outcome = request.send().await;
        // Time to headers, which is the whole of a call that never got any
        // further. It is not what a completed call took: the body is still to
        // come, and a success logged from here reports a request that nearly
        // exhausted its deadline as a fast one (P1).
        let to_headers = started.elapsed();

        let response = match outcome {
            Ok(response) => response,
            Err(source) => {
                warn!(
                    %host,
                    %method,
                    elapsed_ms = to_headers.as_millis(),
                    timeout_ms = timeout_millis,
                    "outbound request did not complete"
                );
                return Err(OutboundError::Unreachable {
                    host,
                    timeout_millis,
                    source,
                });
            }
        };

        let status = response.status();
        // Headers are not an answer. A connection that drops or times out
        // while the body streams has told us nothing, and turning that into an
        // empty body would hand the adapter a "malformed response" it cannot
        // act on while discarding the transport failure that actually
        // happened — the answered-versus-unreachable distinction `I-SRC-1` is
        // built on, collapsed exactly where it matters (P1).
        let body = match response.text().await {
            Ok(body) => body,
            Err(source) => {
                warn!(
                    %host,
                    %method,
                    status = status.as_u16(),
                    elapsed_ms = started.elapsed().as_millis(),
                    timeout_ms = timeout_millis,
                    "outbound response body did not arrive"
                );
                return Err(OutboundError::Unreachable {
                    host,
                    timeout_millis,
                    source,
                });
            }
        };
        // Measured after the body, because the body is the answer. A provider
        // whose headers arrive in 20ms and whose body takes four seconds is a
        // slow provider, and a log that reported 20ms would hide the only
        // number the §21.2 budgets are about.
        let elapsed = started.elapsed();
        info!(
            %host,
            %method,
            status = status.as_u16(),
            elapsed_ms = elapsed.as_millis(),
            "outbound request completed"
        );

        if status.is_success() {
            Ok(Response {
                status: status.as_u16(),
                body,
            })
        } else {
            Err(OutboundError::Status {
                host,
                status: status.as_u16(),
                body: truncated(&body),
            })
        }
    }
}

/// How much of a refusal body is worth keeping for the collapsed detail.
const BODY_EXCERPT_BYTES: usize = 512;

fn truncated(body: &str) -> String {
    if body.len() <= BODY_EXCERPT_BYTES {
        return body.to_owned();
    }
    let mut end = BODY_EXCERPT_BYTES;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    body[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_client_carries_the_default_deadline() {
        let client = OutboundClient::new("afisharr/test").expect("the transport must build");
        assert_eq!(client.deadline(), Deadline::DEFAULT);
    }

    #[test]
    fn a_body_at_the_excerpt_boundary_is_kept_whole() {
        let body = "a".repeat(BODY_EXCERPT_BYTES);
        assert_eq!(truncated(&body).len(), BODY_EXCERPT_BYTES);
    }

    #[test]
    fn a_long_body_is_cut_on_a_character_boundary() {
        let body = "é".repeat(BODY_EXCERPT_BYTES);
        let cut = truncated(&body);
        assert!(cut.len() <= BODY_EXCERPT_BYTES);
        assert!(body.starts_with(&cut));
    }

    #[test]
    fn a_json_body_parses_and_a_malformed_one_names_the_host() {
        let response = Response {
            status: 200,
            body: r#"{"code":"abcd"}"#.to_owned(),
        };
        let parsed: serde_json::Value = response.json("plex.tv").expect("valid JSON");
        assert_eq!(parsed["code"], "abcd");

        let broken = Response {
            status: 200,
            body: "not json".to_owned(),
        };
        let error = broken
            .json::<serde_json::Value>("plex.tv")
            .expect_err("a malformed body must not parse");
        assert_eq!(error.host(), "plex.tv");
    }

    /// A server that promises a body in its headers and then goes away.
    ///
    /// Not a hypothetical shape: an upstream restarted mid-response, or a
    /// proxy that gives up on a slow origin, produces exactly this — a status
    /// line, a `Content-Length`, and a connection that stops.
    async fn a_server_that_stops_mid_body() -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a port must be bindable");
        let address = listener.local_addr().expect("the port must be readable");
        let serving = tokio::spawn(async move {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request).await;
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 64\r\n\r\nhalf")
                .await;
            // Dropped owing sixty more bytes.
        });
        (format!("http://{address}/"), serving)
    }

    #[tokio::test]
    async fn a_body_that_stops_mid_stream_is_unreachable_and_not_an_empty_success() {
        let (url, serving) = a_server_that_stops_mid_body().await;
        let client = OutboundClient::new("afisharr/test").expect("the transport must build");

        let error = client
            .send(
                Method::GET,
                &Url::parse(&url).expect("a valid URL"),
                &[],
                None,
                Deadline::DEFAULT,
            )
            .await
            .expect_err("a body that never arrived must not read as a successful empty one");

        // The distinction the whole error type exists for: this host did not
        // answer, and an adapter that treated it as an empty answer would
        // report "nothing there" about a service it never heard from.
        assert!(
            !error.service_answered(),
            "a dropped body is not an answer: {error}"
        );
        assert!(
            matches!(error, OutboundError::Unreachable { .. }),
            "expected an unreachable host, got {error}"
        );
        assert!(
            std::error::Error::source(&error).is_some(),
            "the transport failure must survive as the source: {error}"
        );

        serving.abort();
    }
}
