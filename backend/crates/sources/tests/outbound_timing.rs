// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the outbound client's success log says a call took.
//!
//! A separate test target because it needs a subscriber: the timing is not
//! returned to the caller, it is reported to the log, and the only honest way
//! to assert on a log line is to collect it.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use afisharr_sources::outbound::{Deadline, Method, OutboundClient};
use tracing::field::{Field, Visit};
use tracing_subscriber::{Layer, layer::SubscriberExt};
use url::Url;

/// How long the stub server holds the body back after sending its headers.
const BODY_DELAY: Duration = Duration::from_millis(400);

/// The fields of one event, as text.
#[derive(Default)]
struct Fields(Vec<(String, String)>);

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.push((field.name().to_owned(), format!("{value:?}")));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push((field.name().to_owned(), value.to_owned()));
    }
}

impl Fields {
    fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Keeps the `elapsed_ms` of the client's completion event.
struct CompletionLog(Arc<Mutex<Option<u128>>>);

impl<S: tracing::Subscriber> Layer<S> for CompletionLog {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        if fields.get("message") != Some("outbound request completed") {
            return;
        }
        let elapsed = fields
            .get("elapsed_ms")
            .and_then(|value| value.parse::<u128>().ok());
        *self.0.lock().expect("the capture must not be poisoned") = elapsed;
    }
}

/// A server that answers its headers at once and its body much later.
///
/// The shape this test exists for: a provider that accepts the request
/// immediately and then takes most of the deadline to produce the answer.
async fn a_server_that_answers_slowly() -> (String, tokio::task::JoinHandle<()>) {
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
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n")
            .await;
        tokio::time::sleep(BODY_DELAY).await;
        let _ = socket.write_all(b"slow").await;
    });
    (format!("http://{address}/"), serving)
}

#[tokio::test]
async fn a_completed_call_is_timed_to_the_end_of_its_body() {
    // Measured before the body, the log reports time-to-headers: a call that
    // spent four hundred milliseconds waiting for the answer is filed as one
    // that took none of them, and a provider that nearly exhausts its deadline
    // looks fast in exactly the log an operator would check.
    let (url, serving) = a_server_that_answers_slowly().await;
    let captured = Arc::new(Mutex::new(None));
    let subscriber = tracing_subscriber::registry().with(CompletionLog(Arc::clone(&captured)));

    let client = OutboundClient::new("afisharr/test").expect("the transport must build");
    // `set_default` rather than `with_default`: the timing happens across await
    // points, and a subscriber installed only for the call that builds the
    // future is gone before the request is sent.
    let _collecting = tracing::subscriber::set_default(subscriber);
    let response = client
        .send(
            Method::GET,
            &Url::parse(&url).expect("a valid URL"),
            &[],
            None,
            Deadline::DEFAULT,
        )
        .await
        .expect("the slow body must still arrive");

    assert_eq!(response.body, "slow");
    let elapsed = captured
        .lock()
        .expect("the capture must not be poisoned")
        .expect("the completion must be logged with an elapsed time");
    assert!(
        elapsed >= BODY_DELAY.as_millis(),
        "the log reported {elapsed}ms for a call whose body took {}ms",
        BODY_DELAY.as_millis()
    );

    serving.abort();
}
