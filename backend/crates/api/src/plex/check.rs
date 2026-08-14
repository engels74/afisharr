// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Running the check, and deciding what it saw.

use afisharr_core::{
    plex_server::{PlexServer, RecordObservation},
    secrets,
    time::Timestamp,
};
use afisharr_plex::server::{
    BindingVerdict, MachineIdentifier, PlexServerClient, ServerAddress, ServerIdentity,
    ServerToken, verify_binding,
};

use crate::{
    error::{AppError, AppResult},
    plex::connection::{PlexConnection, PlexConnectionState},
    state::ApiState,
};

/// The name the Plex server token is sealed under (PRD §19.5).
const PLEX_TOKEN_SECRET: &str = "plex.token";

/// Reads the binding, asks the server who it is, and reports what it saw.
///
/// The whole check, and one round trip. Nothing here reads a library: `I-ID-5`
/// has to be answerable before anything touches a rating key, so the question
/// this asks costs one request against `GET /identity`.
pub(crate) async fn run(state: &ApiState) -> AppResult<PlexConnection> {
    let now = state.clock().now();
    let Some(server) = afisharr_core::plex_server::load(state.database().readers())
        .await
        .map_err(AppError::internal)?
    else {
        return Ok(PlexConnection::not_configured(now.as_millis()));
    };

    let Some(token) = plex_token(state).await? else {
        return Ok(no_credential(&server, now));
    };

    // A stored address that will not parse, or a stored token that cannot be a
    // header, is a server this instance cannot reach — not an internal failure.
    // A 500 here would put an unexplained error on the one page that exists to
    // say what is wrong with the connection, which is the same argument the
    // undecryptable secret above is decided by.
    let client = match client_for(state, &server, &token) {
        Ok(client) => client,
        Err(detail) => return Ok(unreachable(&server, detail, now)),
    };
    match client.identity().await {
        Ok(identity) => Ok(observed(state, &server, identity, now).await?),
        Err(error) => Ok(unreachable(&server, error.to_string(), now)),
    }
}

/// The Plex server token, decrypted, or `None` when none can be presented.
async fn plex_token(state: &ApiState) -> AppResult<Option<String>> {
    let sealed = match secrets::get(
        state.database().readers(),
        state.secret_key(),
        PLEX_TOKEN_SECRET,
    )
    .await
    {
        Ok(sealed) => sealed,
        // A secret that will not decrypt is a database restored without its key
        // (`I-SEC-5`), which is a different problem from having no token at all
        // — but both leave this instance with nothing to present, and both are
        // fixed by signing in to Plex again. The distinction is the restore
        // path's to draw, and it is drawn in Phase 12. Reporting it as an
        // internal failure here would put a 500 on the one page that could have
        // told the operator what to do about it. Nothing is deleted on the
        // strength of it: this reads, and the row stays exactly where it is.
        Err(error @ secrets::SecretError::Undecryptable { .. }) => {
            tracing::warn!(
                %error,
                "the stored Plex token could not be decrypted with the current key; \
                 reporting this instance as having no credential"
            );
            None
        }
        Err(error) => return Err(AppError::internal(error)),
    };
    sealed
        .map(|bytes| String::from_utf8(bytes).map_err(AppError::internal))
        .transpose()
}

/// A client pointed at the bound server, presenting `token`.
///
/// The failure is the collapsed technical detail §8.4 renders, not an error to
/// propagate: neither an address this build cannot parse nor a token it cannot
/// send is something the operator fixes by reloading the page, and both leave
/// this instance unable to reach the server it is bound to.
fn client_for(
    state: &ApiState,
    server: &PlexServer,
    token: &str,
) -> Result<PlexServerClient, String> {
    let address = ServerAddress::parse(&server.base_url).map_err(|error| error.to_string())?;
    let token = ServerToken::new(token).map_err(|error| error.to_string())?;
    Ok(PlexServerClient::new(
        state.outbound().clone(),
        state.plex().identity().clone(),
        address,
        token,
    ))
}

/// The answer for a bound server this instance has no credential for.
fn no_credential(server: &PlexServer, now: Timestamp) -> PlexConnection {
    PlexConnection {
        state: PlexConnectionState::NoCredential,
        base_url: Some(server.base_url.clone()),
        bound_machine_identifier: Some(server.machine_identifier.clone()),
        observed_machine_identifier: None,
        friendly_name: Some(server.friendly_name.clone()),
        version: Some(server.version.clone()),
        detail: None,
        checked_at: now.as_millis(),
    }
}

/// The answer for a server that did not answer, or answered unusably.
fn unreachable(server: &PlexServer, detail: String, now: Timestamp) -> PlexConnection {
    PlexConnection {
        state: PlexConnectionState::Unreachable,
        base_url: Some(server.base_url.clone()),
        bound_machine_identifier: Some(server.machine_identifier.clone()),
        // Deliberately none. Something may have answered — a proxy, a captive
        // portal, a different service on the port — and whatever it said is not
        // a machine identifier. Reporting one here would be reporting an
        // observation that was never made (P1).
        observed_machine_identifier: None,
        friendly_name: Some(server.friendly_name.clone()),
        version: Some(server.version.clone()),
        detail: Some(detail),
        checked_at: now.as_millis(),
    }
}

/// The answer for a server that answered, and the write that follows it.
async fn observed(
    state: &ApiState,
    server: &PlexServer,
    identity: ServerIdentity,
    now: Timestamp,
) -> AppResult<PlexConnection> {
    let bound = MachineIdentifier::new(server.machine_identifier.clone());
    let verdict = verify_binding(Some(&bound), &identity.machine_identifier);

    let blocked = verdict.blocks();
    let answer = PlexConnection {
        state: if blocked {
            PlexConnectionState::WrongServer
        } else {
            PlexConnectionState::Reachable
        },
        base_url: Some(server.base_url.clone()),
        bound_machine_identifier: Some(server.machine_identifier.clone()),
        observed_machine_identifier: Some(identity.machine_identifier.to_string()),
        // The fallback is the *bound* server's recorded name, and `GET /identity`
        // never carries one — so on a blocked verdict it would pair the old
        // server's name with the stranger's version and present the two as one
        // server describing itself (P1). Nobody named the machine that answered,
        // so this answer does not either.
        friendly_name: identity.friendly_name.clone().or_else(|| {
            if blocked {
                None
            } else {
                Some(server.friendly_name.clone())
            }
        }),
        version: Some(identity.version.clone()),
        detail: None,
        checked_at: now.as_millis(),
    };

    if matches!(verdict, BindingVerdict::DifferentServer { .. }) {
        // Zero writes. Not even `last_seen_at`: the row describes the server
        // this installation is bound to, and touching any of it on the strength
        // of a stranger's answer is the beginning of the silent rebind
        // `I-ID-5` exists to forbid.
        tracing::warn!(
            expected = %server.machine_identifier,
            found = %identity.machine_identifier,
            "a different Plex server answered at the bound address; nothing was written"
        );
        return Ok(answer);
    }

    state
        .database()
        .writer()
        .submit(RecordObservation {
            machine_identifier: identity.machine_identifier.to_string(),
            friendly_name: identity
                .friendly_name
                .unwrap_or_else(|| server.friendly_name.clone()),
            version: identity.version,
            platform: identity.platform.or_else(|| server.platform.clone()),
            base_url: server.base_url.clone(),
            at: now,
        })
        .await
        .map_err(AppError::internal)?;
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use afisharr_plex::server::ServerError;
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

    #[test]
    fn an_unreachable_answer_reports_no_observed_identifier_at_all() {
        // Whatever answered on that port, it did not name a machine. An answer
        // carrying one here would be an observation nobody made (P1).
        let error = ServerError::Transport {
            host: "plex.lan".to_owned(),
            source: OutboundError::Status {
                host: "plex.lan".to_owned(),
                status: 502,
                body: String::new(),
            },
        };
        let answer = unreachable(&server(), error.to_string(), Timestamp::from_millis(3_000));
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
    }

    #[test]
    fn a_bound_server_with_no_credential_is_its_own_answer() {
        let answer = no_credential(&server(), Timestamp::from_millis(3_000));
        assert_eq!(answer.state, PlexConnectionState::NoCredential);
        assert_eq!(answer.base_url.as_deref(), Some("http://plex.lan:32400/"));
        assert_eq!(answer.observed_machine_identifier, None);
        assert_eq!(answer.detail, None);
    }
}
