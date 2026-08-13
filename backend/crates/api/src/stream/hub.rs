// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The fan-out every connection subscribes to.

use tokio::sync::{broadcast, watch};

use crate::stream::StreamEvent;

/// How often the stream emits a heartbeat.
///
/// Published to the client in the opening event so the disconnection watchdog
/// is derived from the server's interval rather than from a number the two
/// sides have to keep in step by hand. `I-UX-9` asks for the indicator within
/// one missed heartbeat, so the client waits a little over this and then says
/// so.
pub const HEARTBEAT_SECONDS: u64 = 15;

/// How many events a slow subscriber may fall behind before it is dropped.
///
/// A subscriber that overflows this is told it lagged, and the client's answer
/// is to refetch — which is what it would do on a reconnect anyway. Buffering
/// without bound to protect a subscriber that is not reading would trade a
/// stale tab for the instance's memory.
const BACKLOG: usize = 256;

/// The publish side of the stream.
///
/// Cloneable and cheap: every subsystem that publishes holds one, and every
/// connection holds a subscription taken from one.
#[derive(Debug, Clone)]
pub struct StreamHub {
    sender: broadcast::Sender<StreamEvent>,
    closing: watch::Sender<bool>,
}

impl StreamHub {
    /// A hub with no subscribers.
    #[must_use]
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BACKLOG);
        Self {
            sender,
            closing: watch::channel(false).0,
        }
    }

    /// Publishes one event to every current subscriber.
    ///
    /// Returns how many received it. Zero is not an error: an instance nobody
    /// has a tab open on publishes into nothing, and the surfaces that event
    /// would have accelerated are correct on their next load regardless.
    #[must_use = "zero subscribers is a fact worth acting on, or explicitly ignoring"]
    pub fn publish(&self, event: StreamEvent) -> usize {
        self.sender.send(event).unwrap_or(0)
    }

    /// Subscribes one connection.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.sender.subscribe()
    }

    /// How many connections are currently subscribed.
    #[must_use]
    pub fn subscribers(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Ends every open connection, and every one opened after this.
    ///
    /// Called when the process is asked to stop. A graceful shutdown stops
    /// accepting connections and then waits for the responses already in
    /// flight — and an event stream's response body never ends on its own, so
    /// one browser with a tab open is the difference between a clean stop and
    /// a container killed at the end of its grace period.
    pub fn close(&self) {
        // `send_replace`, not `send`: `send` fails and leaves the value
        // untouched when nothing is subscribed, which is precisely the
        // instance with no stream open — and a connection accepted during the
        // drain would then never learn the stream had been closed.
        let _ = self.closing.send_replace(true);
    }

    /// Completes when the stream is closed, for one connection to end on.
    ///
    /// `use<>` rather than a bare `impl Future`: in edition 2024 an opaque
    /// return type captures the lifetime of `&self` unless it says otherwise,
    /// and this future is handed to a response body that outlives both the
    /// handler and the borrow it was built from.
    pub fn closed(&self) -> impl Future<Output = ()> + Send + use<> {
        let mut closing = self.closing.subscribe();
        async move {
            // `wait_for` inspects the current value before it waits, which is
            // what makes a connection opened after the close end at once
            // rather than hang until the drain deadline.
            let _ = closing.wait_for(|closed| *closed).await;
        }
    }
}

impl Default for StreamHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::stream::Topic;

    use super::*;

    fn event(payload: &str) -> StreamEvent {
        StreamEvent {
            topic: Topic::Jobs,
            payload: payload.to_owned(),
        }
    }

    #[tokio::test]
    async fn a_subscriber_receives_what_is_published_after_it_subscribed() {
        let hub = StreamHub::new();
        let mut subscriber = hub.subscribe();
        assert_eq!(hub.publish(event("{}")), 1);
        assert_eq!(
            subscriber.recv().await.expect("the event must arrive"),
            event("{}")
        );
    }

    #[tokio::test]
    async fn publishing_with_no_subscribers_is_not_a_failure() {
        let hub = StreamHub::new();
        assert_eq!(hub.publish(event("{}")), 0);
        assert_eq!(hub.subscribers(), 0);
    }

    #[tokio::test]
    async fn every_subscriber_receives_every_event() {
        let hub = StreamHub::new();
        let mut first = hub.subscribe();
        let mut second = hub.subscribe();
        assert_eq!(hub.publish(event(r#"{"n":1}"#)), 2);
        assert_eq!(first.recv().await.expect("delivered").payload, r#"{"n":1}"#);
        assert_eq!(
            second.recv().await.expect("delivered").payload,
            r#"{"n":1}"#
        );
    }

    #[tokio::test]
    async fn a_subscriber_that_falls_too_far_behind_is_told_it_lagged() {
        // Being told is what matters: a silently truncated stream looks like a
        // working one, and the client's answer to a lag is the same refetch it
        // performs after a reconnect (I-UX-9).
        let hub = StreamHub::new();
        let mut subscriber = hub.subscribe();
        for n in 0..(BACKLOG + 10) {
            let _ = hub.publish(event(&format!(r#"{{"n":{n}}}"#)));
        }
        assert!(matches!(
            subscriber.recv().await,
            Err(broadcast::error::RecvError::Lagged(_))
        ));
    }

    #[tokio::test]
    async fn an_open_connection_ends_when_the_stream_is_closed() {
        // The stop this makes possible: a graceful shutdown waits for the
        // responses already in flight, and an event stream's body never ends
        // on its own. Without this the process never reaches its own cleanup.
        let hub = StreamHub::new();
        let open = hub.closed();
        hub.close();
        tokio::time::timeout(std::time::Duration::from_secs(5), open)
            .await
            .expect("an open connection must end when the stream closes");
    }

    #[tokio::test]
    async fn a_connection_opened_after_the_close_ends_at_once() {
        // The race a shutdown actually runs into: a request accepted a
        // moment before the signal builds its body a moment after it.
        let hub = StreamHub::new();
        hub.close();
        tokio::time::timeout(std::time::Duration::from_secs(5), hub.closed())
            .await
            .expect("a late connection must not wait for the drain deadline");
    }

    #[tokio::test]
    async fn a_connection_stays_open_while_the_stream_is_not_closing() {
        let hub = StreamHub::new();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), hub.closed())
                .await
                .is_err(),
            "nothing has asked the stream to stop"
        );
    }
}
