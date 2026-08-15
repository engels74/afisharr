// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a key is allowed to do, as opposed to who issued it.
//!
//! A key used to carry no answer to that question, so the guard answered it
//! from the creator instead: a key issued by an administrator *was* an
//! administrator, on every route, including the one that issues more keys. An
//! operator who wanted a token for one integration to read the filesystem had
//! no way to ask for less, and the integration that leaked it handed over the
//! whole instance — the Plex connection, every session, and the ability to mint
//! a replacement credential that survives revoking the first (Task 1.3).
//!
//! So a key names its capabilities when it is issued, they are stored beside
//! its digest, and each route says which one it needs. A scope is a ceiling and
//! never a grant: the account behind the key still has to hold the rights the
//! route asks for, and the scope only ever takes away.

/// One capability an API key may be granted.
///
/// Deliberately coarse, and one per thing an operator would describe out loud —
/// "let it browse my files", "let it read the event stream". A scope per route
/// would be a second copy of the route table, kept by hand, and the copy that
/// drifts is the one that decides who gets in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scope {
    /// Browsing the filesystem: `GET /api/files` and `GET /api/files/roots`.
    FilesRead,
    /// Reading the event stream: `GET /api/stream`.
    EventsRead,
    /// Listing and revoking the account's own sessions.
    SessionsManage,
    /// Changing the account's own password.
    AccountManage,
    /// Listing, issuing, and revoking API keys.
    ///
    /// Grantable, and never implied. A key holding this can issue another that
    /// outlives its own revocation, which is a thing an operator may genuinely
    /// want and must never get by accident.
    KeysManage,
    /// Reading the Plex connection, and checking it.
    ///
    /// Its own scope rather than folded into another, because what it reaches
    /// is a request to the operator's own Plex server: a key issued to browse
    /// the filesystem has no business making this instance talk to Plex.
    PlexRead,
}

impl Scope {
    /// Every scope, in the order the interface lists them.
    pub const ALL: [Self; 6] = [
        Self::FilesRead,
        Self::EventsRead,
        Self::SessionsManage,
        Self::AccountManage,
        Self::KeysManage,
        Self::PlexRead,
    ];

    /// The name a caller writes and the interface shows.
    ///
    /// `noun:verb`, as every other product's tokens are written, so an operator
    /// reading a key's scopes in Settings recognises the shape.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FilesRead => "files:read",
            Self::EventsRead => "events:read",
            Self::SessionsManage => "sessions:manage",
            Self::AccountManage => "account:manage",
            Self::KeysManage => "keys:manage",
            Self::PlexRead => "plex:read",
        }
    }

    /// The scope `text` names, if this binary knows one.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|scope| scope.as_str() == text)
    }

    /// This scope's place in the stored bitset.
    const fn bit(self) -> u8 {
        match self {
            Self::FilesRead => 1,
            Self::EventsRead => 1 << 1,
            Self::SessionsManage => 1 << 2,
            Self::AccountManage => 1 << 3,
            Self::KeysManage => 1 << 4,
            Self::PlexRead => 1 << 5,
        }
    }
}

/// The capabilities one key holds.
///
/// A set and not a list: asking twice for the same scope is asking once, and
/// the order an operator typed them in is not a fact worth storing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScopeSet(u8);

impl ScopeSet {
    /// A key that may do nothing.
    pub const NONE: Self = Self(0);

    /// The set holding exactly `scopes`.
    #[must_use]
    pub fn of(scopes: impl IntoIterator<Item = Scope>) -> Self {
        Self(scopes.into_iter().fold(0, |bits, scope| bits | scope.bit()))
    }

    /// Whether this set holds `scope`.
    #[must_use]
    pub const fn contains(self, scope: Scope) -> bool {
        self.0 & scope.bit() != 0
    }

    /// Whether this set holds nothing at all.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The scopes held, in [`Scope::ALL`]'s order.
    #[must_use]
    pub fn held(self) -> Vec<Scope> {
        Scope::ALL
            .into_iter()
            .filter(|scope| self.contains(*scope))
            .collect()
    }

    /// The names held, in [`Scope::ALL`]'s order.
    #[must_use]
    pub fn names(self) -> Vec<String> {
        self.held()
            .into_iter()
            .map(|scope| scope.as_str().to_owned())
            .collect()
    }

    /// This set as the one string `api_keys.scopes` holds.
    ///
    /// Space-separated names rather than the bitset itself, because the column
    /// is read by an operator with `sqlite3` at least as often as by this
    /// binary, and a number tells them nothing. It is also what keeps a bit
    /// re-ordering from silently re-permissioning every key on the instance.
    #[must_use]
    pub fn stored(self) -> String {
        self.held()
            .into_iter()
            .map(Scope::as_str)
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The set `text` holds, as [`Self::stored`] wrote it.
    ///
    /// A name this binary does not know is dropped rather than kept, which is
    /// the safe direction: a key written by a newer Afisharr and read by an
    /// older one holds fewer capabilities here, never more. A downgrade is
    /// refused before this is ever reached (`MigrationState::ensure_openable`);
    /// this is what the answer would be if it were not.
    #[must_use]
    pub fn parse_stored(text: &str) -> Self {
        Self::of(text.split_whitespace().filter_map(Scope::parse))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scope_has_a_distinct_name_and_a_distinct_bit() {
        let mut names: Vec<&str> = Scope::ALL.into_iter().map(Scope::as_str).collect();
        names.sort_unstable();
        let mut deduplicated = names.clone();
        deduplicated.dedup();
        assert_eq!(names, deduplicated, "two scopes share a name");

        // Two scopes sharing a bit is one scope granting the other, silently.
        let combined = ScopeSet::of(Scope::ALL);
        assert_eq!(combined.held().len(), Scope::ALL.len());
    }

    #[test]
    fn a_name_round_trips_through_parse() {
        for scope in Scope::ALL {
            assert_eq!(Scope::parse(scope.as_str()), Some(scope));
        }
        assert_eq!(Scope::parse("files:write"), None);
        assert_eq!(Scope::parse("*"), None);
    }

    #[test]
    fn a_set_holds_what_it_was_given_and_nothing_else() {
        let set = ScopeSet::of([Scope::FilesRead]);
        assert!(set.contains(Scope::FilesRead));
        for scope in Scope::ALL {
            if scope != Scope::FilesRead {
                assert!(!set.contains(scope), "{scope:?} was never asked for");
            }
        }
    }

    #[test]
    fn an_empty_set_grants_nothing() {
        assert!(ScopeSet::NONE.is_empty());
        for scope in Scope::ALL {
            assert!(!ScopeSet::NONE.contains(scope));
        }
    }

    #[test]
    fn a_set_round_trips_through_the_column_it_is_stored_in() {
        let set = ScopeSet::of([Scope::FilesRead, Scope::KeysManage]);
        assert_eq!(set.stored(), "files:read keys:manage");
        assert_eq!(ScopeSet::parse_stored(&set.stored()), set);
        assert_eq!(ScopeSet::parse_stored(""), ScopeSet::NONE);
    }

    #[test]
    fn a_stored_name_this_binary_does_not_know_grants_nothing() {
        // The safe direction. A row written by a newer binary must not be read
        // as "everything", and an operator's typo in the column must not be
        // read as anything at all.
        let read = ScopeSet::parse_stored("files:read instance:destroy");
        assert_eq!(read, ScopeSet::of([Scope::FilesRead]));
    }
}
