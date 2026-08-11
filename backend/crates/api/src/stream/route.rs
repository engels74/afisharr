// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `GET /api/stream` — the one connection.

use std::{convert::Infallible, time::Duration};

use axum::{
    extract::State,
    response::sse::{Event, Sse},
};
use tokio_stream::{
    Stream, StreamExt,
    wrappers::{BroadcastStream, IntervalStream, errors::BroadcastStreamRecvError},
};

use crate::{
    authentication::Administrator,
    state::ApiState,
    stream::{HEARTBEAT_SECONDS, Topic},
};

/// What the connection says first.
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamOpened {
    /// How often a heartbeat arrives, in seconds.
    ///
    /// The client derives its disconnection watchdog from this rather than
    /// carrying a copy of the interval, so the two cannot drift apart.
    pub heartbeat_seconds: u64,
    /// Every topic this connection carries.
    pub topics: Vec<Topic>,
}

/// Opens the multiplexed event stream.
///
/// Requires authentication, like every other route on this surface — the
/// stream carries job progress and source health, which name the operator's
/// libraries and integrations.
///
/// The stream never replays. A client that reconnects has missed events and
/// knows it; the answer is to refetch the surfaces it feeds, which is what a
/// fresh page load does anyway (PRD §9). Building a replay buffer would make
/// two paths to the same state, and the one that is exercised less would be
/// the one that is wrong (P7).
#[utoipa::path(
    get,
    path = "/api/stream",
    tag = "stream",
    responses(
        (status = 200, description = "The event stream, multiplexed by topic", content_type = "text/event-stream"),
        (status = 401, description = "No accepted credential was presented", body = crate::error::Problem),
        (status = 403, description = "That account does not administer this instance, or setup has not been completed", body = crate::error::Problem),
        (status = 429, description = "Too many requests", body = crate::error::Problem),
    ),
)]
pub async fn stream(
    State(state): State<ApiState>,
    _caller: Administrator,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let opened = Event::default().event(Topic::Stream.as_event_name()).data(
        serde_json::to_string(&StreamOpened {
            heartbeat_seconds: HEARTBEAT_SECONDS,
            topics: vec![Topic::Stream, Topic::Jobs, Topic::Sources],
        })
        .unwrap_or_else(|_| String::from("{}")),
    );

    let published = BroadcastStream::new(state.stream().subscribe()).filter_map(|received| {
        match received {
            Ok(event) => Some(Ok(Event::default()
                .event(event.topic.as_event_name())
                .data(event.payload))),
            // A lagged subscriber is told, on the stream's own topic, so the
            // client refetches instead of quietly carrying stale numbers.
            Err(BroadcastStreamRecvError::Lagged(_)) => Some(Ok(Event::default()
                .event(Topic::Stream.as_event_name())
                .data(r#"{"lagged":true}"#))),
        }
    });

    // A named event on the stream's own topic, not `KeepAlive`. Axum's keep-alive
    // writes an SSE *comment*, and a browser's `EventSource` never dispatches a
    // comment frame to a listener — so on an idle but perfectly healthy stream
    // no client listener is ever called, every watchdog expires, and every
    // client reports a disconnection that has not happened (`I-UX-9`).
    let mut ticker = tokio::time::interval(Duration::from_secs(HEARTBEAT_SECONDS));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let heartbeat = IntervalStream::new(ticker).skip(1).map(|_| {
        Ok(Event::default()
            .event(Topic::Stream.as_event_name())
            .data(r#"{"heartbeat":true}"#))
    });

    // Ended by the shutdown signal, not only by the client hanging up. A
    // graceful stop waits for every response already in flight, and this body
    // is one that never finishes on its own: without an end of its own, a
    // single tab holding `/api/stream` open keeps the process past its
    // container's grace period and into a forced kill, with the database never
    // closed.
    //
    // `Option` because `merge` carries one item type and `map_while` stops at
    // the first `None`, which is what the close resolves into.
    let hub = state.stream().clone();
    let events = tokio_stream::once(Ok(opened))
        .chain(published.merge(heartbeat))
        .map(Some);
    let closed = tokio_stream::once(())
        .then(move |()| hub.closed())
        .map(|()| None);
    let body = events.merge(closed).map_while(|event| event);

    Sse::new(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_opening_event_names_every_topic_and_the_heartbeat() {
        let opened = StreamOpened {
            heartbeat_seconds: HEARTBEAT_SECONDS,
            topics: vec![Topic::Stream, Topic::Jobs, Topic::Sources],
        };
        let encoded = serde_json::to_value(&opened).expect("serialises");
        assert_eq!(encoded["heartbeatSeconds"], 15);
        assert_eq!(
            encoded["topics"],
            serde_json::json!(["stream", "jobs", "sources"])
        );
    }
}
