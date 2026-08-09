// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The fan-out every connection subscribes to.

use tokio::sync::broadcast;

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
}

impl StreamHub {
    /// A hub with no subscribers.
    #[must_use]
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BACKLOG);
        Self { sender }
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
}
