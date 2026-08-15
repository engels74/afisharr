// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What each outcome of the check is reported as.
//!
//! Split from the check itself because the two are answerable separately: what
//! the calls the check makes produce is one question, and what an operator is
//! told about it is another. Everything here decides the second,
//! and every function in it withholds more than it reports — a failing check
//! observed nothing, and an answer that filled the gaps from the stored row
//! would present weeks-old facts as what the server just said (P1).

use afisharr_core::{plex_server::PlexServer, time::Timestamp};
use afisharr_plex::server::{ServerError, ServerIdentity, redact_credentials};

use crate::plex::connection::{PlexConnection, PlexConnectionState};

/// Whether this failure is the server rejecting the credential presented.
///
/// 401 and 403 and nothing else. A 404 is a server that does not serve this
/// path, and a 5xx is the server's own failure; neither is fixed by signing in
/// to Plex again, which is the whole remedy [`credential_refused`] names.
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
/// The stored address is whatever the operator configured, and an operator
/// whose server sits behind a reverse proxy configures
/// `http://user:secret@plex.lan`. This field is read by the browser and
/// rendered on the settings page, so the password comes off here — the request
/// itself is built from the stored text and still carries it.
pub(crate) fn shown_address(server: &PlexServer) -> String {
    redact_credentials(&server.base_url)
}

/// The answer for a bound server this instance has no credential for.
pub(crate) fn no_credential(server: &PlexServer, now: Timestamp) -> PlexConnection {
    PlexConnection {
        state: PlexConnectionState::NoCredential,
        base_url: Some(shown_address(server)),
        bound_machine_identifier: Some(server.machine_identifier.clone()),
        observed_machine_identifier: None,
        // Nothing was asked, so nothing was reported. `friendly_name` and
        // `version` are what the *server said about itself on this check*, and
        // the interface renders them as exactly that — so filling them from the
        // stored row would present a name and a version last seen weeks ago as
        // something the server just told us (P1).
        friendly_name: None,
        version: None,
        detail: None,
        checked_at: now.as_millis(),
    }
}

/// The answer for the bound server, answering, with the token accepted.
///
/// All three conditions, and the third is not free: the identity call behind
/// `identity` answers before authentication, so a caller that reported this
/// state on the strength of it alone would report a revoked token as a working
/// connection.
pub(crate) fn reachable(
    server: &PlexServer,
    identity: &ServerIdentity,
    now: Timestamp,
) -> PlexConnection {
    PlexConnection {
        state: PlexConnectionState::Reachable,
        base_url: Some(shown_address(server)),
        bound_machine_identifier: Some(server.machine_identifier.clone()),
        observed_machine_identifier: Some(identity.machine_identifier.to_string()),
        // `GET /identity` carries no name, and this is the server the row
        // describes — so the stored name is the operator's own answer to "which
        // machine is this", not a fact invented here.
        friendly_name: identity
            .friendly_name
            .clone()
            .or_else(|| Some(server.friendly_name.clone())),
        version: Some(identity.version.clone()),
        detail: None,
        checked_at: now.as_millis(),
    }
}

/// The answer for a *different* server answering at the bound address.
///
/// Names both identifiers, because the decision it hands back to the operator
/// needs both: an answer naming only the stranger says nothing about what they
/// are being asked to abandon (`I-ID-5`).
pub(crate) fn wrong_server(
    server: &PlexServer,
    identity: &ServerIdentity,
    now: Timestamp,
) -> PlexConnection {
    PlexConnection {
        state: PlexConnectionState::WrongServer,
        base_url: Some(shown_address(server)),
        bound_machine_identifier: Some(server.machine_identifier.clone()),
        observed_machine_identifier: Some(identity.machine_identifier.to_string()),
        // No fallback to the stored name here, unlike [`reachable`]. The stored
        // name belongs to the *bound* server, and pairing it with the
        // stranger's version would present the two as one machine describing
        // itself (P1). Nobody named the machine that answered.
        friendly_name: identity.friendly_name.clone(),
        version: Some(identity.version.clone()),
        detail: None,
        checked_at: now.as_millis(),
    }
}

/// The answer for a server that did not answer, or answered unusably.
pub(crate) fn unreachable(server: &PlexServer, detail: String, now: Timestamp) -> PlexConnection {
    without_a_usable_answer(server, PlexConnectionState::Unreachable, detail, now)
}

/// The answer for a server that answered by refusing the credential.
///
/// Everything it withholds, it withholds for the same reason [`unreachable`]
/// does: a refusal names no machine and describes no server, so this answer
/// does not either (P1).
pub(crate) fn credential_refused(
    server: &PlexServer,
    detail: String,
    now: Timestamp,
) -> PlexConnection {
    without_a_usable_answer(server, PlexConnectionState::CredentialRefused, detail, now)
}

/// The shape both failing answers share.
fn without_a_usable_answer(
    server: &PlexServer,
    state: PlexConnectionState,
    detail: String,
    now: Timestamp,
) -> PlexConnection {
    PlexConnection {
        state,
        base_url: Some(shown_address(server)),
        bound_machine_identifier: Some(server.machine_identifier.clone()),
        // Deliberately none. Something may have answered — a proxy, a captive
        // portal, a different service on the port — and whatever it said is not
        // a machine identifier. Reporting one here would be reporting an
        // observation that was never made (P1).
        observed_machine_identifier: None,
        // Nothing described itself, so this answer describes nothing. The
        // stored name and version would otherwise sit directly under "the
        // server did not answer" and read as what it just said — the same
        // observation-that-never-happened the identifier above refuses.
        friendly_name: None,
        version: None,
        detail: Some(detail),
        checked_at: now.as_millis(),
    }
}

#[cfg(test)]
mod tests {
    use afisharr_plex::server::MachineIdentifier;
    use afisharr_sources::outbound::OutboundError;

    use super::*;

    fn server() -> PlexServer {
        PlexServer {
            machine_identifier: "server-a".to_owned(),
            friendly_name: "Living Room".to_owned(),
            version: "1.41.0".to_owned(),
            platform: Some("Linux".to_owned()),
            base_url: "http://plex.lan:32400/".to_owned(),
            owner_account_id: None,
            first_seen_at: Timestamp::from_millis(1_000),
            last_seen_at: Timestamp::from_millis(2_000),
            last_version_change_at: None,
        }
    }

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

    fn identity(machine_identifier: &str) -> ServerIdentity {
        ServerIdentity {
            machine_identifier: MachineIdentifier::new(machine_identifier),
            version: "1.41.9".to_owned(),
            // `GET /identity` carries no name, which is the case both builders
            // below have to differ on.
            friendly_name: None,
            platform: Some("Linux".to_owned()),
        }
    }

    #[test]
    fn a_reachable_answer_names_the_server_the_row_describes() {
        let answer = reachable(
            &server(),
            &identity("server-a"),
            Timestamp::from_millis(3_000),
        );
        assert_eq!(answer.state, PlexConnectionState::Reachable);
        assert_eq!(
            answer.observed_machine_identifier.as_deref(),
            Some("server-a")
        );
        assert_eq!(
            answer.friendly_name.as_deref(),
            Some("Living Room"),
            "the identity call carries no name, and this is the bound server"
        );
        assert_eq!(answer.version.as_deref(), Some("1.41.9"));
        assert_eq!(answer.detail, None);
    }

    #[test]
    fn a_wrong_server_answer_names_both_machines_and_borrows_no_name() {
        // The stored name belongs to the bound server. Paired with the
        // stranger's version it would read as one machine describing itself,
        // which is an observation nobody made (P1).
        let answer = wrong_server(
            &server(),
            &identity("server-b"),
            Timestamp::from_millis(3_000),
        );
        assert_eq!(answer.state, PlexConnectionState::WrongServer);
        assert_eq!(answer.bound_machine_identifier.as_deref(), Some("server-a"));
        assert_eq!(
            answer.observed_machine_identifier.as_deref(),
            Some("server-b")
        );
        assert_eq!(answer.friendly_name, None);
        assert_eq!(answer.version.as_deref(), Some("1.41.9"));
    }

    #[test]
    fn a_name_the_server_reported_is_preferred_to_the_stored_one() {
        let mut named = identity("server-a");
        named.friendly_name = Some("Basement".to_owned());
        let answer = reachable(&server(), &named, Timestamp::from_millis(3_000));
        assert_eq!(answer.friendly_name.as_deref(), Some("Basement"));
    }

    #[test]
    fn an_unreachable_answer_reports_no_observed_identifier_at_all() {
        // Whatever answered on that port, it did not name a machine. An answer
        // carrying one here would be an observation nobody made (P1).
        let error = refusal(502);
        let answer = unreachable(&server(), detail_of(&error), Timestamp::from_millis(3_000));
        assert_eq!(answer.state, PlexConnectionState::Unreachable);
        assert_eq!(answer.observed_machine_identifier, None);
        assert_eq!(
            answer.bound_machine_identifier.as_deref(),
            Some("server-a"),
            "the operator still needs to know what it was looking for"
        );
        assert!(
            answer
                .detail
                .is_some_and(|detail| detail.contains("plex.lan")),
            "the collapsed technical detail names the host"
        );
        assert_eq!(
            (answer.friendly_name, answer.version),
            (None, None),
            "nothing described itself, so this answer describes nothing"
        );
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
    fn a_server_that_refuses_the_token_is_not_a_server_that_did_not_answer() {
        // Opposite remedies: one is a network fault to chase, the other is a
        // sign-in to repeat. An operator sent to the first for the second
        // spends the evening on a fault that is not there (`I-UX-2`).
        for status in [401, 403] {
            let error = refusal(status);
            assert!(refused_credential(&error), "{status}");
            let answer =
                credential_refused(&server(), detail_of(&error), Timestamp::from_millis(3_000));
            assert_eq!(answer.state, PlexConnectionState::CredentialRefused);
            assert_eq!(answer.bound_machine_identifier.as_deref(), Some("server-a"));
            // A refusal names no machine and describes no server (P1).
            assert_eq!(answer.observed_machine_identifier, None);
            assert_eq!((answer.friendly_name, answer.version), (None, None));
            assert!(
                answer
                    .detail
                    .is_some_and(|detail| detail.contains(&status.to_string()))
            );
        }
    }

    #[test]
    fn a_failure_that_is_not_a_refusal_stays_unreachable() {
        // 404 is a server that does not serve this path and 503 is the
        // server's own trouble. Neither is fixed by signing in to Plex again,
        // so neither may borrow the state whose whole remedy is that.
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

        let answer = no_credential(&behind_a_proxy, Timestamp::from_millis(3_000));
        let shown = answer.base_url.expect("the address is shown");
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
    fn a_bound_server_with_no_credential_is_its_own_answer() {
        let answer = no_credential(&server(), Timestamp::from_millis(3_000));
        assert_eq!(answer.state, PlexConnectionState::NoCredential);
        assert_eq!(answer.base_url.as_deref(), Some("http://plex.lan:32400/"));
        assert_eq!(answer.observed_machine_identifier, None);
        assert_eq!(answer.detail, None);
        // Nothing was asked, so nothing was reported (P1).
        assert_eq!((answer.friendly_name, answer.version), (None, None));
    }
}
