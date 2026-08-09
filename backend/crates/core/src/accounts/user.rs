// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One row of `users`, as the rest of the product reads it.

use crate::time::Timestamp;

/// How an account proves who it is.
///
/// A closed set with an exhaustive `match` at every use, because the schema's
/// `CHECK ((kind = 'Local') = (password_hash IS NOT NULL))` makes the two kinds
/// structurally different rows rather than one row with an optional field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserKind {
    /// A username and an Argon2id password held by this instance.
    Local,
    /// A plex.tv account, proved by a token this instance stores in `secrets`.
    Plex,
}

impl UserKind {
    /// The text stored in `users.kind`.
    #[must_use]
    pub const fn as_text(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::Plex => "Plex",
        }
    }

    /// Reads the value back from the column.
    #[must_use]
    pub fn from_text(text: &str) -> Option<Self> {
        match text {
            "Local" => Some(Self::Local),
            "Plex" => Some(Self::Plex),
            _ => None,
        }
    }
}

/// An account that may sign in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    /// ULID primary key.
    pub id: String,
    /// Which proof this account carries.
    pub kind: UserKind,
    /// The name typed at the login form, unique across accounts.
    pub username: String,
    /// Contact address, when one is known.
    pub email: Option<String>,
    /// The name shown in the interface, when it differs from the username.
    pub display_name: Option<String>,
    /// The Argon2id PHC string, present only for [`UserKind::Local`].
    pub password_hash: Option<String>,
    /// plex.tv's numeric account id, present only for [`UserKind::Plex`].
    pub plex_account_id: Option<i64>,
    /// plex.tv's account uuid.
    pub plex_uuid: Option<String>,
    /// Avatar the interface shows beside the account.
    pub avatar_url: Option<String>,
    /// Whether this account holds the one trust level the surface has (D-007).
    pub is_admin: bool,
    /// When the row was created.
    pub created_at: Timestamp,
    /// When the row was last changed.
    pub updated_at: Timestamp,
    /// When this account last signed in.
    pub last_login_at: Option<Timestamp>,
    /// When this account was disabled, if it was.
    pub disabled_at: Option<Timestamp>,
}

impl User {
    /// Whether this account may sign in at all.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.disabled_at.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_round_trips_through_its_column_text() {
        for kind in [UserKind::Local, UserKind::Plex] {
            assert_eq!(UserKind::from_text(kind.as_text()), Some(kind));
        }
    }

    #[test]
    fn a_kind_the_schema_does_not_allow_does_not_parse() {
        assert_eq!(UserKind::from_text("Administrator"), None);
    }

    #[test]
    fn a_disabled_account_is_not_active() {
        let user = User {
            id: "U".to_owned(),
            kind: UserKind::Local,
            username: "operator".to_owned(),
            email: None,
            display_name: None,
            password_hash: Some("$argon2id$".to_owned()),
            plex_account_id: None,
            plex_uuid: None,
            avatar_url: None,
            is_admin: true,
            created_at: Timestamp::EPOCH,
            updated_at: Timestamp::EPOCH,
            last_login_at: None,
            disabled_at: Some(Timestamp::from_millis(1)),
        };
        assert!(!user.is_active());
    }
}
