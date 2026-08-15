// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The enumerated values a tag filter offers, and their fast keys.

use afisharr_sources::outbound::Method;
use serde::Deserialize;

use crate::{
    discovery::DiscoveredFilter,
    server::{PlexServerClient, ServerError},
};

/// One value a filter can take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterChoice {
    /// The value to send in a query, e.g. a genre's tag id.
    pub value: String,
    /// The label the server shows for it.
    pub title: Option<String>,
    /// The endpoint that lists matching items directly, when the server offers
    /// one. It is a shortcut, never the only way to the same set.
    pub fast_key: Option<String>,
}

/// The choice list Plex answers with.
#[derive(Debug, Deserialize)]
pub(crate) struct ChoicesBody {
    #[serde(default, rename = "Directory")]
    pub(crate) directory: Vec<ChoiceBody>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChoiceBody {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    fast_key: Option<String>,
}

impl From<ChoiceBody> for Option<FilterChoice> {
    fn from(body: ChoiceBody) -> Self {
        // A choice with no key cannot be sent in a query, so it is dropped
        // rather than given the title as a value: the title is what the
        // operator reads, and Plex matches on the key.
        let value = body.key.filter(|key| !key.is_empty())?;
        Some(FilterChoice {
            value,
            title: body.title.filter(|title| !title.is_empty()),
            fast_key: body.fast_key.filter(|key| !key.is_empty()),
        })
    }
}

impl PlexServerClient {
    /// Reads the enumerated choices of one discovered filter.
    ///
    /// The filter's own `key` is the endpoint — the server composed it, query
    /// string and all, so nothing here reassembles a path from parts that could
    /// disagree with what the server said (P7).
    ///
    /// It is also the one endpoint in this crate a *server* names, so it is the
    /// one that has to be checked before it is requested: the request carries
    /// this instance's `X-Plex-Token`, and an absolute key would point it off
    /// this machine. `discovered_endpoint` is where that check lives.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer,
    /// [`ServerError::Incomplete`] when the filter declared no choice endpoint
    /// — a free-value filter has no list, which is not an empty one — and
    /// [`ServerError::ForeignEndpoint`] when the key names another server.
    #[tracing::instrument(skip(self))]
    pub async fn filter_choices(
        &self,
        filter: &DiscoveredFilter,
    ) -> Result<Vec<FilterChoice>, ServerError> {
        let key = filter.key.as_deref().ok_or(ServerError::Incomplete {
            call: "GET a filter's choice list",
            missing: "an endpoint to read the choices from",
        })?;
        let url = self.discovered_endpoint(key)?;
        let body: ChoicesBody = self.container(Method::GET, &url, None).await?;
        Ok(body
            .directory
            .into_iter()
            .filter_map(Option::<FilterChoice>::from)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A client bound to an address nothing is listening on: every test here
    /// asserts a request was refused before it was sent, so a reachable server
    /// would make the assertion pass for the wrong reason.
    fn client() -> crate::server::PlexServerClient {
        crate::server::PlexServerClient::new(
            afisharr_sources::outbound::OutboundClient::new("afisharr/test")
                .expect("the transport must build"),
            crate::identity::ClientIdentity::new("01JABCDEF", "Living Room", "0.1.0")
                .expect("a valid identity"),
            crate::server::ServerAddress::parse("http://127.0.0.1:1").expect("a valid address"),
            crate::server::ServerToken::new("plex-token").expect("a header-safe token"),
        )
    }

    fn choices(json: &str) -> Vec<FilterChoice> {
        let body: ChoicesBody = serde_json::from_str(json).expect("parses");
        body.directory
            .into_iter()
            .filter_map(Option::<FilterChoice>::from)
            .collect()
    }

    #[test]
    fn a_choice_reads_its_value_title_and_fast_key() {
        let choices = choices(
            r#"{"Directory":[{"key":"93","title":"Comedy",
                "fastKey":"/library/sections/1/all?genre=93"}]}"#,
        );
        assert_eq!(choices[0].value, "93");
        assert_eq!(choices[0].title.as_deref(), Some("Comedy"));
        assert_eq!(
            choices[0].fast_key.as_deref(),
            Some("/library/sections/1/all?genre=93")
        );
    }

    #[test]
    fn a_choice_with_no_fast_key_is_still_a_choice() {
        // The fast key is a shortcut. A client that required one would drop
        // every value on a server version that stopped sending them.
        let choices = choices(r#"{"Directory":[{"key":"93","title":"Comedy"}]}"#);
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].fast_key, None);
    }

    #[test]
    fn a_choice_with_no_key_is_dropped_rather_than_keyed_on_its_title() {
        // Plex matches on the key. A title used as a value produces a filter
        // that silently matches nothing.
        assert!(choices(r#"{"Directory":[{"title":"Comedy"}]}"#).is_empty());
    }

    #[test]
    fn an_empty_choice_list_parses_as_an_empty_list() {
        assert!(choices(r#"{"size":0}"#).is_empty());
    }

    #[tokio::test]
    async fn a_filter_pointing_at_another_host_is_refused_before_the_token_is_sent() {
        // The key is a string out of a response body, and this request carries
        // the instance's `X-Plex-Token`. A server that answered with somebody
        // else's URL — or anything that rewrote the answer on the way — would
        // otherwise be handed the credential (D-032).
        let client = client();
        let filter = DiscoveredFilter {
            filter: "genre".to_owned(),
            filter_type: "string".to_owned(),
            title: None,
            key: Some("http://collector.example/library/sections/1/genre".to_owned()),
        };
        let error = client
            .filter_choices(&filter)
            .await
            .expect_err("another host is not this server");
        assert!(
            matches!(error, ServerError::ForeignEndpoint { .. }),
            "{error}"
        );
        assert!(!error.server_answered(), "no request was made");
        assert!(error.to_string().contains("collector.example"), "{error}");
    }

    #[tokio::test]
    async fn a_free_value_filter_is_refused_before_a_request_is_made() {
        // `key: None` means the filter takes a typed value. Requesting `""`
        // would resolve to the server root and parse its answer as a choice
        // list, which is an empty vocabulary reported as a fact (P1).
        let client = client();
        let filter = DiscoveredFilter {
            filter: "year".to_owned(),
            filter_type: "integer".to_owned(),
            title: None,
            key: None,
        };
        let error = client
            .filter_choices(&filter)
            .await
            .expect_err("a free-value filter has no choice list");
        assert!(matches!(error, ServerError::Incomplete { .. }));
    }
}
