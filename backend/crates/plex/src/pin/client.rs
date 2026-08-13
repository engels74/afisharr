// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Creating a pin, and polling it.

use afisharr_sources::outbound::{HeaderValue, Method, OutboundClient, OutboundError};
use url::Url;

use crate::{
    account::{AccountBody, PlexAccount},
    identity::{ClientIdentity, PLEX_TOKEN},
    pin::{PinError, PinPoll, PinResource, resource::PinBody},
};

/// plex.tv's API root.
const PLEX_TV_BASE: &str = "https://plex.tv/api/v2";

/// What a pin reports as its client identifier when it reports none.
///
/// A placeholder rather than an empty string, because it is read by an operator
/// in the refusal `PinError::ClientIdentifierMismatch` renders, and "under
/// client identifier ''" reads as a bug in Afisharr rather than as an answer
/// plex.tv did not give.
const ABSENT_IDENTIFIER: &str = "(none given)";

/// The plex.tv side of the login flow.
///
/// Holds no token: a pin exchange is how a token is obtained, so a client that
/// needed one to run would have nowhere to start.
#[derive(Debug, Clone)]
pub struct PlexTvClient {
    outbound: OutboundClient,
    identity: ClientIdentity,
    base: String,
}

impl PlexTvClient {
    /// A client that identifies as `identity` and sends through `outbound`.
    #[must_use]
    pub fn new(outbound: OutboundClient, identity: ClientIdentity) -> Self {
        Self::against(outbound, identity, PLEX_TV_BASE)
    }

    /// The same client pointed at another API root.
    ///
    /// plex.tv is the only value this takes in production. It is a parameter
    /// because the flow is otherwise untestable without the real service, and
    /// because the adversarial fake (D-036) is the thing every later phase
    /// tests against.
    #[must_use]
    pub fn against(outbound: OutboundClient, identity: ClientIdentity, base: &str) -> Self {
        Self {
            outbound,
            identity,
            base: base.trim_end_matches('/').to_owned(),
        }
    }

    /// The identity every request from this client carries.
    #[must_use]
    pub const fn identity(&self) -> &ClientIdentity {
        &self.identity
    }

    /// Creates a pin resource and returns the code to present.
    ///
    /// `strong` asks plex.tv for a long code rather than the four-character
    /// one; the OAuth variant uses it because the operator never types it, and
    /// a code nobody reads may as well be unguessable.
    ///
    /// # Errors
    /// Returns [`PinError::Transport`] when plex.tv did not answer,
    /// [`PinError::NoIdentifier`] when it answered without a pollable id, and
    /// [`PinError::ClientIdentifierMismatch`] when it recorded the pin under a
    /// different client identifier than this instance sent.
    #[tracing::instrument(skip(self))]
    pub async fn create_pin(&self, strong: bool) -> Result<PinResource, PinError> {
        let mut url = Url::parse(&format!("{}/pins", self.base)).map_err(|source| {
            PinError::Transport(OutboundError::Address {
                host: "plex.tv".to_owned(),
                source,
            })
        })?;
        url.query_pairs_mut()
            .append_pair("strong", if strong { "true" } else { "false" });

        let response = self
            .outbound
            .send(
                Method::POST,
                &url,
                &self.identity.headers(),
                None,
                self.outbound.deadline(),
            )
            .await?;
        let body: PinBody = response.json("plex.tv")?;

        let plex_pin_id = body.identifier().ok_or(PinError::NoIdentifier)?;
        // An answer that does not say which client the pin belongs to is not a
        // match, and defaulting it to our own identifier made the check below
        // compare a value with itself: it could not fire, and the same
        // substituted value went into `plex_pin_logins`, so the second guard on
        // the poll passed too. plex.tv dropping the field — a version change on
        // their side is enough — then meant the operator completed the whole
        // sign-in and was handed a token every later call refuses, which is the
        // opaque failure this variant exists to prevent (PRD §19.6).
        let recorded = body
            .client_identifier
            .clone()
            .unwrap_or_else(|| ABSENT_IDENTIFIER.to_owned());
        if recorded != self.identity.client_identifier() {
            return Err(PinError::ClientIdentifierMismatch {
                expected: self.identity.client_identifier().to_owned(),
                found: recorded,
            });
        }

        Ok(PinResource {
            plex_pin_id,
            code: body.code.clone(),
            client_identifier: recorded,
            expires_in_seconds: body.expires_in_seconds(),
        })
    }

    /// Polls one pin.
    ///
    /// A 404 from plex.tv means the pin is gone, which is
    /// [`PinPoll::Expired`] and not a transport failure — the service answered.
    /// Anything else that is not an answer stays an error, so a network outage
    /// during a login is never reported to the operator as "your code expired".
    ///
    /// # Errors
    /// Returns [`PinError::Transport`] when plex.tv did not answer.
    #[tracing::instrument(skip(self))]
    pub async fn poll_pin(&self, plex_pin_id: &str) -> Result<PinPoll, PinError> {
        let url = Url::parse(&format!("{}/pins/{plex_pin_id}", self.base)).map_err(|source| {
            PinError::Transport(OutboundError::Address {
                host: "plex.tv".to_owned(),
                source,
            })
        })?;

        let response = match self
            .outbound
            .send(
                Method::GET,
                &url,
                &self.identity.headers(),
                None,
                self.outbound.deadline(),
            )
            .await
        {
            Ok(response) => response,
            Err(OutboundError::Status { status: 404, .. }) => return Ok(PinPoll::Expired),
            Err(other) => return Err(PinError::Transport(other)),
        };

        let body: PinBody = response.json("plex.tv")?;
        Ok(match body.auth_token {
            Some(auth_token) if !auth_token.is_empty() => PinPoll::Authorized { auth_token },
            _ => PinPoll::Pending,
        })
    }

    /// Reads the account a token authenticates.
    ///
    /// The step between "a token arrived" and "this is who signed in". Without
    /// it a completed pin proves only that somebody, somewhere, has a plex.tv
    /// account — which is not a fact any instance should act on.
    ///
    /// # Errors
    /// Returns [`PinError::Transport`] when plex.tv did not answer, or answered
    /// a refusal, which for this call means the token is not accepted.
    #[tracing::instrument(skip(self, auth_token))]
    pub async fn account(&self, auth_token: &str) -> Result<PlexAccount, PinError> {
        let url = Url::parse(&format!("{}/user", self.base)).map_err(|source| {
            PinError::Transport(OutboundError::Address {
                host: "plex.tv".to_owned(),
                source,
            })
        })?;

        let mut headers = self.identity.headers();
        headers.push((
            PLEX_TOKEN,
            HeaderValue::from_str(auth_token).map_err(|_| PinError::NoIdentifier)?,
        ));

        let response = self
            .outbound
            .send(Method::GET, &url, &headers, None, self.outbound.deadline())
            .await?;
        let body: AccountBody = response.json("plex.tv")?;
        Ok(PlexAccount::from(body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> PlexTvClient {
        PlexTvClient::new(
            OutboundClient::new("afisharr/test").expect("the transport must build"),
            ClientIdentity::new("01JABCDEF", "Living Room", "0.1.0").expect("a valid identity"),
        )
    }

    #[test]
    fn the_client_reports_the_identity_it_will_send() {
        assert_eq!(client().identity().client_identifier(), "01JABCDEF");
    }

    #[test]
    fn a_body_with_a_matching_identifier_yields_a_resource() {
        let body: PinBody = serde_json::from_str(
            r#"{"id":42,"code":"abcd","clientIdentifier":"01JABCDEF","expiresIn":900}"#,
        )
        .expect("parses");
        assert_eq!(body.identifier().as_deref(), Some("42"));
        assert_eq!(body.client_identifier.as_deref(), Some("01JABCDEF"));
        assert_eq!(body.expires_in_seconds(), 900);
    }

    #[test]
    fn an_authorised_body_carries_the_token_and_a_pending_one_does_not() {
        let authorised: PinBody =
            serde_json::from_str(r#"{"id":42,"code":"abcd","authToken":"tok"}"#).expect("parses");
        assert_eq!(authorised.auth_token.as_deref(), Some("tok"));

        let pending: PinBody =
            serde_json::from_str(r#"{"id":42,"code":"abcd","authToken":null}"#).expect("parses");
        assert_eq!(pending.auth_token, None);
    }
}
