// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `GET /api/v2/user`, and what it says.

use serde::Deserialize;

/// The plex.tv account a token authenticates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlexAccount {
    /// plex.tv's numeric account id. The binding a `users` row is matched on.
    pub id: i64,
    /// plex.tv's account uuid.
    pub uuid: Option<String>,
    /// The account's username.
    pub username: String,
    /// The account's email address, when plex.tv reports one.
    pub email: Option<String>,
    /// The avatar, when plex.tv reports one.
    pub thumb: Option<String>,
}

/// The account body exactly as plex.tv's JSON carries it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountBody {
    pub(crate) id: i64,
    #[serde(default)]
    pub(crate) uuid: Option<String>,
    #[serde(default)]
    pub(crate) username: Option<String>,
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) email: Option<String>,
    #[serde(default)]
    pub(crate) thumb: Option<String>,
}

impl From<AccountBody> for PlexAccount {
    fn from(body: AccountBody) -> Self {
        Self {
            id: body.id,
            uuid: body.uuid,
            // `username` is absent on managed accounts, where `title` is the
            // only name there is. Falling back keeps a linked managed account
            // from displaying as a blank row.
            username: body
                .username
                .or(body.title)
                .unwrap_or_else(|| format!("plex-{}", body.id)),
            email: body.email,
            thumb: body.thumb,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ordinary_account_reads_its_username() {
        let body: AccountBody = serde_json::from_str(
            r#"{"id":12345,"uuid":"u","username":"operator","email":"a@b.c"}"#,
        )
        .expect("parses");
        let account = PlexAccount::from(body);
        assert_eq!(account.id, 12345);
        assert_eq!(account.username, "operator");
        assert_eq!(account.email.as_deref(), Some("a@b.c"));
    }

    #[test]
    fn a_managed_account_falls_back_to_its_title() {
        let body: AccountBody = serde_json::from_str(r#"{"id":7,"title":"Kids"}"#).expect("parses");
        assert_eq!(PlexAccount::from(body).username, "Kids");
    }

    #[test]
    fn an_account_with_no_name_at_all_is_still_identifiable() {
        let body: AccountBody = serde_json::from_str(r#"{"id":7}"#).expect("parses");
        assert_eq!(PlexAccount::from(body).username, "plex-7");
    }

    #[test]
    fn a_field_plex_adds_later_does_not_break_the_parse() {
        let body: AccountBody =
            serde_json::from_str(r#"{"id":7,"newThing":[1,2]}"#).expect("parses");
        assert_eq!(body.id, 7);
    }
}
