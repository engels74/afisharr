// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a failure and an address are shown as.
//!
//! Split from the answers themselves because the two questions are separable:
//! which of §8.1's six states an outcome is belongs next to the states, and how
//! the two free-text fields inside one are rendered belongs here. The split is
//! also where the credential handling lives, and that is easier to audit as one
//! short file than as a preamble to six builders — every function below is on
//! the path from a stored row or an error chain to the browser, and each one
//! takes a secret out on the way (D-032).

use afisharr_core::plex_server::PlexServer;
use afisharr_plex::server::{ServerAddress, ServerError, redact_credentials};

/// Whether this failure is the server rejecting the credential presented.
///
/// 401 and 403 and nothing else. A 404 is a server that does not serve this
/// path, and a 5xx is the server's own failure; neither is fixed by signing in
/// to Plex again, which is the whole remedy `credential_refused` names.
pub(crate) fn refused_credential(error: &ServerError) -> bool {
    matches!(error.refused_status(), Some(401 | 403))
}

/// A failure and everything under it, as one line.
///
/// The whole chain, because the outer message is the part that says least:
/// every transport failure renders as "the Plex server at {host} could not be
/// reached", and what tells a timeout from a refused token from a proxy's own
/// error page is the `#[source]` beneath it. Collapsing to `to_string()` put the
/// same sentence in §8.4's collapsed detail whatever went wrong, which is a
/// detail that details nothing.
///
/// Redacted on the way out. This string is rendered on the page and pasted into
/// bug reports, and a layer of it can quote the configured address — which, for
/// an operator whose server sits behind basic auth, carries a password.
pub(crate) fn detail_of(error: &dyn std::error::Error) -> String {
    let mut detail = error.to_string();
    let mut cause = error.source();
    while let Some(source) = cause {
        let text = source.to_string();
        // Skipped when the layer below only restates the layer above, which is
        // what `reqwest` does for the outermost of its own wrappers.
        if !detail.ends_with(&text) {
            detail.push_str(": ");
            detail.push_str(&text);
        }
        cause = source.source();
    }
    redact_credentials(&detail)
}

/// The bound address, as an answer may show it.
///
/// The stored address is whatever the operator configured, and it can carry a
/// credential two ways. An operator whose server sits behind a reverse proxy
/// configures `http://user:secret@plex.lan`, and an operator who copied the URL
/// out of Plex Web configures `http://plex:32400/?X-Plex-Token=secret`. This
/// field is read by the browser and rendered on the settings page, so both come
/// off here — the request itself is built from the stored text and still
/// carries the password.
///
/// Both are removed by parsing rather than by hand, so the rule about what a
/// server address may hold stays in the one type that states it:
/// [`ServerAddress`] drops the query and the fragment and redacts the password.
/// Doing it here from the stored string instead would be a second answer to the
/// same question, free to drift from the first. The fallback is for a stored
/// row [`ServerAddress`] cannot parse, which the check itself reports as
/// unreachable — the address is still shown, and still without its password.
pub(crate) fn shown_address(server: &PlexServer) -> String {
    ServerAddress::parse(&server.base_url).map_or_else(
        |_| redact_credentials(&server.base_url),
        |address| address.as_str().to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use afisharr_sources::outbound::OutboundError;

    use super::*;
    use crate::plex::answer::tests::server;

    fn refusal(status: u16) -> ServerError {
        ServerError::Transport {
            host: "plex.lan".to_owned(),
            source: OutboundError::Status {
                host: "plex.lan".to_owned(),
                status,
                body: String::new(),
            },
        }
    }

    #[test]
    fn the_collapsed_detail_carries_what_went_wrong_and_not_only_that_something_did() {
        // Every transport failure's own message is the same sentence, so a
        // detail built from it alone tells a refused token, an expired
        // certificate, and a proxy's error page apart from nothing at all — and
        // §8.4's collapsed detail exists for exactly that distinction.
        let detail = detail_of(&refusal(401));
        assert!(detail.contains("401"), "{detail}");

        let unread = ServerError::Transport {
            host: "plex.lan".to_owned(),
            source: OutboundError::Oversized {
                host: "plex.lan".to_owned(),
                limit_bytes: 1024,
            },
        };
        assert!(detail_of(&unread).contains("1024"), "{unread}");

        // And a failure with nothing under it is still exactly itself.
        let incomplete = ServerError::Incomplete {
            call: "GET /identity",
            missing: "a machine identifier",
        };
        assert_eq!(detail_of(&incomplete), incomplete.to_string());
    }

    #[test]
    fn a_refusal_is_told_apart_from_every_other_way_a_call_can_fail() {
        // Opposite remedies: one is a network fault to chase, the other is a
        // sign-in to repeat. An operator sent to the first for the second
        // spends the evening on a fault that is not there (`I-UX-2`).
        for status in [401, 403] {
            assert!(refused_credential(&refusal(status)), "{status}");
        }
        // 404 is a server that does not serve this path and 503 is the
        // server's own trouble. Neither is fixed by signing in to Plex again.
        for status in [404, 429, 500, 503] {
            assert!(!refused_credential(&refusal(status)), "{status}");
        }
        assert!(!refused_credential(&ServerError::Incomplete {
            call: "GET /identity",
            missing: "a machine identifier",
        }));
    }

    #[test]
    fn a_password_in_the_configured_address_reaches_neither_the_browser_nor_the_detail() {
        // An operator whose server sits behind basic auth configures
        // `http://user:secret@plex.lan`, and this route hands `baseUrl` and
        // the collapsed detail straight to the page.
        let mut behind_a_proxy = server();
        behind_a_proxy.base_url = "http://admin:hunter2@plex.lan:32400/".to_owned();

        let shown = shown_address(&behind_a_proxy);
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(shown.contains("plex.lan"), "{shown}");

        // And the same address quoted back inside a failure message, which is
        // how `AddressError` names what the operator typed.
        let quoted = std::io::Error::other(format!(
            "'{}' is not a URL",
            behind_a_proxy.base_url.trim_end_matches('/')
        ));
        let detail = detail_of(&quoted);
        assert!(!detail.contains("hunter2"), "{detail}");
        assert!(detail.contains("admin:***@plex.lan"), "{detail}");
    }

    #[test]
    fn a_token_in_the_configured_address_does_not_reach_the_browser_either() {
        // The other way a credential arrives in a base: an operator who copied
        // the URL out of Plex Web copied the token with it. Redacting only
        // userinfo left this one intact, and this field is rendered on the
        // settings page (D-032).
        let mut copied_from_plex_web = server();
        copied_from_plex_web.base_url = "http://plex.lan:32400/?X-Plex-Token=hunter2".to_owned();

        let shown = shown_address(&copied_from_plex_web);
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(!shown.contains("X-Plex-Token"), "{shown}");
        assert!(shown.contains("plex.lan"), "{shown}");
    }

    #[test]
    fn an_address_this_build_cannot_parse_is_still_shown_without_its_password() {
        // The check reports this row as unreachable, and the operator still has
        // to see what they typed in order to fix it.
        let mut mistyped = server();
        mistyped.base_url = "http://admin:hunter2@ plex.lan".to_owned();
        let shown = shown_address(&mistyped);
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(shown.contains("admin:***@"), "{shown}");
    }

    #[test]
    fn an_address_with_nothing_to_hide_is_shown_as_it_was_configured() {
        assert_eq!(shown_address(&server()), "http://plex.lan:32400/");
    }
}
