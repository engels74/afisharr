// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What travels on the stream.

use serde::Serialize;
use utoipa::ToSchema;

/// The topics one connection is multiplexed over.
///
/// A closed set rather than a free string: a subscriber that filters on a topic
/// nobody publishes silently receives nothing, and the failure looks exactly
/// like a working stream with no activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum Topic {
    /// The stream itself: the opening event and the heartbeat.
    Stream,
    /// Progress of a job run.
    Jobs,
    /// A source's health changing.
    Sources,
}

impl Topic {
    /// The SSE `event:` name a subscriber listens for.
    #[must_use]
    pub const fn as_event_name(self) -> &'static str {
        match self {
            Self::Stream => "stream",
            Self::Jobs => "jobs",
            Self::Sources => "sources",
        }
    }
}

/// One published event.
///
/// The payload is already-serialised JSON rather than a typed body, because
/// each topic's shape belongs to the subsystem that publishes it and this
/// module must not grow a variant per feature (§24.6.3). The topic is the
/// contract; the payload's schema is the publisher's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamEvent {
    /// Which topic this belongs to.
    pub topic: Topic,
    /// The JSON body, as text.
    pub payload: String,
}

impl StreamEvent {
    /// An event on `topic` carrying `payload`.
    ///
    /// # Errors
    /// Returns the serialisation failure. A payload that will not serialise is
    /// a publisher's bug, and dropping it silently would present as a stream
    /// that works for every topic but one.
    pub fn of<T: Serialize>(topic: Topic, payload: &T) -> Result<Self, serde_json::Error> {
        Ok(Self {
            topic,
            payload: serde_json::to_string(payload)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_topic_has_a_distinct_event_name() {
        let names = [Topic::Stream, Topic::Jobs, Topic::Sources]
            .map(Topic::as_event_name)
            .to_vec();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len());
    }

    #[test]
    fn an_event_carries_its_payload_as_json_text() {
        let event = StreamEvent::of(Topic::Jobs, &serde_json::json!({ "runId": "01J" }))
            .expect("serialises");
        assert_eq!(event.topic, Topic::Jobs);
        assert_eq!(event.payload, r#"{"runId":"01J"}"#);
    }

    #[test]
    fn topics_serialise_as_camel_case_names() {
        assert_eq!(
            serde_json::to_string(&Topic::Sources).expect("serialises"),
            "\"sources\""
        );
    }
}
