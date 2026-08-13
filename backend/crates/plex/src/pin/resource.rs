// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What plex.tv says a pin is, and what a poll of one found.

use serde::Deserialize;

/// A pin resource as plex.tv creates it.
///
/// `client_identifier` is carried on the value rather than assumed, because a
/// pin issued under a different identifier yields a token that will not work
/// and the failure is otherwise entirely opaque (PRD §19.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinResource {
    /// plex.tv's identifier for the pin, used to poll it.
    pub plex_pin_id: String,
    /// The four-character code the operator types at plex.tv/link.
    pub code: String,
    /// The client identifier the pin was created under.
    pub client_identifier: String,
    /// How many seconds from creation the pin stops being pollable.
    pub expires_in_seconds: i64,
}

/// What one poll of a pin found.
///
/// Three states, not two: a pin that has not been authorised yet and a pin that
/// will never be authorised are different facts, and treating "no token" as
/// "expired" would abandon a flow the operator is halfway through (P1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinPoll {
    /// The operator has not finished signing in. Poll again.
    Pending,
    /// A token was issued.
    Authorized {
        /// The token, which goes straight to `secrets` and nowhere else.
        auth_token: String,
    },
    /// The pin's window closed without a token.
    Expired,
}

/// The pin resource exactly as plex.tv's JSON carries it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PinBody {
    pub(crate) id: serde_json::Value,
    pub(crate) code: String,
    #[serde(default)]
    pub(crate) client_identifier: Option<String>,
    #[serde(default)]
    pub(crate) expires_in: Option<i64>,
    #[serde(default)]
    pub(crate) auth_token: Option<String>,
}

/// How long plex.tv's pins live when it does not say.
///
/// Fifteen minutes is plex.tv's documented default. Substituting it is safe in
/// the direction P2 requires — the poll stops early rather than running past a
/// window that has actually closed.
const DEFAULT_EXPIRY_SECONDS: i64 = 15 * 60;

impl PinBody {
    /// The identifier as text, whatever JSON shape it arrived in.
    ///
    /// plex.tv has answered both `"id": 12345` and `"id": "12345"` across
    /// versions, and a client that accepts only one of them breaks on an
    /// upgrade nobody here controls.
    ///
    /// A string form is held to the shape an identifier can actually have,
    /// because this value is pasted into the poll URL's path
    /// (`{base}/pins/{id}`). A value carrying a `/`, a `?`, or a `#` is not a
    /// path segment: `"id": "../user"` normalises the poll to
    /// `https://plex.tv/api/v2/user`, which answers 200 with a body carrying no
    /// `authToken`, so every poll reads `Pending` and the operator watches a
    /// sign-in that can never finish with nothing anywhere saying why. Refused
    /// here, it is [`PinError::NoIdentifier`] — the answer this crate already
    /// gives for an identifier it cannot follow.
    ///
    /// [`PinError::NoIdentifier`]: crate::pin::PinError::NoIdentifier
    pub(crate) fn identifier(&self) -> Option<String> {
        match &self.id {
            serde_json::Value::String(text) => is_pollable(text).then(|| text.clone()),
            serde_json::Value::Number(number) => Some(number.to_string()),
            _ => None,
        }
    }

    /// How long the pin lives, in seconds, as a window that can actually be
    /// used.
    ///
    /// A zero or negative `expiresIn` is treated as absent rather than obeyed.
    /// It is not a shorter window, it is a window that closed before it opened,
    /// and it does not stay upstream: it becomes the attempt row's `expires_at`
    /// and the `Max-Age` of the cookie that binds the attempt to this browser.
    /// Emitted at zero, the browser discards that cookie on arrival, every poll
    /// answers "that sign-in was started somewhere else", and starting again
    /// reproduces it — Plex sign-in broken for good, by a message that blames
    /// the browser for a value plex.tv sent.
    pub(crate) fn expires_in_seconds(&self) -> i64 {
        self.expires_in
            .filter(|seconds| *seconds > 0)
            .unwrap_or(DEFAULT_EXPIRY_SECONDS)
    }
}

/// Whether an identifier is one this client can put in a URL path unchanged.
///
/// Deliberately narrow: every identifier plex.tv has ever issued is a decimal
/// number, and the tolerance here exists only so a future alphanumeric one is
/// not refused. Anything else is a value that would change which endpoint the
/// poll reaches rather than which pin it names.
fn is_pollable(identifier: &str) -> bool {
    !identifier.is_empty()
        && identifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_numeric_identifier_reads_as_text() {
        let body: PinBody = serde_json::from_str(r#"{"id":12345,"code":"abcd"}"#).expect("parses");
        assert_eq!(body.identifier().as_deref(), Some("12345"));
    }

    #[test]
    fn a_string_identifier_reads_as_itself() {
        let body: PinBody =
            serde_json::from_str(r#"{"id":"12345","code":"abcd"}"#).expect("parses");
        assert_eq!(body.identifier().as_deref(), Some("12345"));
    }

    #[test]
    fn an_identifier_of_an_unexpected_shape_is_absent_rather_than_invented() {
        let body: PinBody = serde_json::from_str(r#"{"id":null,"code":"abcd"}"#).expect("parses");
        assert_eq!(body.identifier(), None);
    }

    #[test]
    fn an_identifier_that_is_not_a_path_segment_is_refused() {
        // The failure this closes: the identifier is pasted into the poll URL's
        // path, so `"../user"` normalises the poll to plex.tv's `/user`
        // endpoint. That answers 200 with no `authToken`, so every poll reads
        // `Pending` for ever and the operator watches a sign-in that cannot
        // finish, with nothing anywhere saying why.
        for id in ["../user", "42?X-Plex-Token=x", "42#frag", "42/43", ""] {
            let body: PinBody =
                serde_json::from_str(&format!(r#"{{"id":{},"code":"abcd"}}"#, json_string(id)))
                    .expect("parses");
            assert_eq!(body.identifier(), None, "id {id:?}");
        }
    }

    #[test]
    fn an_alphanumeric_identifier_a_later_version_might_issue_is_still_accepted() {
        // The bound on the rule above: it exists to keep the value a path
        // segment, not to insist plex.tv keeps issuing decimal numbers.
        let body: PinBody =
            serde_json::from_str(r#"{"id":"pin_abc-123","code":"abcd"}"#).expect("parses");
        assert_eq!(body.identifier().as_deref(), Some("pin_abc-123"));
    }

    fn json_string(value: &str) -> String {
        serde_json::Value::String(value.to_owned()).to_string()
    }

    #[test]
    fn an_absent_expiry_falls_back_to_the_documented_default() {
        let body: PinBody = serde_json::from_str(r#"{"id":1,"code":"abcd"}"#).expect("parses");
        assert_eq!(body.expires_in_seconds(), DEFAULT_EXPIRY_SECONDS);
    }

    #[test]
    fn a_window_that_closed_before_it_opened_falls_back_too() {
        // Obeyed, this reaches the attempt cookie as `Max-Age=0`: the browser
        // discards it on arrival and every poll of the sign-in the operator is
        // looking at answers that it was started somewhere else.
        for expiry in ["0", "-1", "-900"] {
            let body: PinBody =
                serde_json::from_str(&format!(r#"{{"id":1,"code":"abcd","expiresIn":{expiry}}}"#))
                    .expect("parses");
            assert_eq!(
                body.expires_in_seconds(),
                DEFAULT_EXPIRY_SECONDS,
                "expiresIn {expiry}"
            );
        }
    }

    #[test]
    fn a_field_plex_adds_later_does_not_break_the_parse() {
        // Provider responses are parsed defensively: plex.tv adds fields
        // without warning, and refusing the body over one is an outage.
        let body: PinBody =
            serde_json::from_str(r#"{"id":1,"code":"abcd","newThing":true}"#).expect("parses");
        assert_eq!(body.code, "abcd");
    }
}
