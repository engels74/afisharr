// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! May they do this, given that they are who they say they are.
//!
//! Separate from `guard`, which answers the first question. The split is the
//! one the extractors themselves make: signing out and reading one's own
//! session need only [`Authenticated`], and everything else needs a named
//! capability as well.
//!
//! The capability is a type parameter rather than a call inside the handler,
//! and that is the whole design. A route that forgot to check its scope would
//! compile and answer, and the failure is invisible in every test that uses a
//! session — a session holds every scope. Writing it as
//! `Administrator<FilesRead>` means the route table states each route's
//! requirement in the one place a reader is already looking, and a new route
//! cannot be added without naming one.

use afisharr_core::api_keys::Scope;
use axum::{extract::FromRequestParts, http::request::Parts};

use crate::{
    authentication::Authenticated,
    error::{AppError, ErrorCode},
    state::ApiState,
};

/// The capability one route asks of the credential presented to it.
pub trait Requires {
    /// The scope an API key must hold to reach the route.
    const SCOPE: Scope;
}

/// Names a marker type for one scope, and documents it as the route table sees
/// it.
macro_rules! requirement {
    ($(#[$doc:meta])* $name:ident => $scope:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy)]
        pub struct $name;

        impl Requires for $name {
            const SCOPE: Scope = $scope;
        }
    };
}

requirement! {
    /// Browsing the filesystem.
    FilesRead => Scope::FilesRead
}
requirement! {
    /// Reading the event stream.
    EventsRead => Scope::EventsRead
}
requirement! {
    /// Listing and revoking the account's own sessions.
    SessionsManage => Scope::SessionsManage
}
requirement! {
    /// Changing the account's own password.
    AccountManage => Scope::AccountManage
}
requirement! {
    /// Listing, issuing, and revoking API keys.
    KeysManage => Scope::KeysManage
}
requirement! {
    /// Reading the Plex connection, and checking it.
    PlexRead => Scope::PlexRead
}

/// A caller who has proved who they are, on a route their credential reaches.
///
/// For the self-scoped routes: they act on the calling account and no other, so
/// they ask no administrator rights — but a key issued to browse the filesystem
/// still has no business changing the account's password.
#[derive(Debug, Clone)]
pub struct Scoped<R: Requires>(
    /// The caller, once their credential is judged wide enough.
    pub Authenticated,
    /// Carries the requirement into the type. Never read.
    pub std::marker::PhantomData<R>,
);

impl<R: Requires> FromRequestParts<ApiState> for Scoped<R> {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ApiState,
    ) -> Result<Self, Self::Rejection> {
        let caller = Authenticated::from_request_parts(parts, state).await?;
        permit(&caller, R::SCOPE)?;
        Ok(Self(caller, std::marker::PhantomData))
    }
}

/// A caller who has proved who they are, **holds administrator rights**, and
/// presented a credential wide enough for the route.
///
/// Tier 0 is an admin-only product (D-007): the filesystem browser, the
/// instance's API keys, the Plex connection, and the event stream are one
/// operator's control panel over their own server, and none of them is scoped
/// to the account that asked. `users.is_admin` can still be `0` — a Plex
/// account linked for viewing, a row edited by hand — and such an account holds
/// a session this surface accepts. Without this extractor the whole documented
/// admin-only surface is ordinary authenticated access.
///
/// The two questions are asked in this order for the reason the refusals
/// differ: "your account does not administer this instance" is a fact about the
/// operator, and "that key was not issued for this" is a fact about the
/// credential in their hand. An operator who is told the first when the second
/// is true goes looking at the wrong thing.
#[derive(Debug, Clone)]
pub struct Administrator<R: Requires>(
    /// The caller, once their rights and their credential are established.
    pub Authenticated,
    /// Carries the requirement into the type. Never read.
    pub std::marker::PhantomData<R>,
);

impl<R: Requires> FromRequestParts<ApiState> for Administrator<R> {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ApiState,
    ) -> Result<Self, Self::Rejection> {
        let caller = Authenticated::from_request_parts(parts, state).await?;
        if !caller.is_admin {
            return Err(AppError::of(
                ErrorCode::Forbidden,
                "That account does not administer this instance.",
            ));
        }
        permit(&caller, R::SCOPE)?;
        Ok(Self(caller, std::marker::PhantomData))
    }
}

/// Refuses a credential that was not issued to reach `scope`.
///
/// The scope is named in the message. A key is held by a script whose author is
/// reading a log, not by somebody who can go and look at the interface, and
/// "forbidden" alone leaves them guessing which capability to re-issue it with.
/// Naming it discloses nothing: the caller already holds the key, and this is
/// the list they chose from when they made it.
fn permit(caller: &Authenticated, scope: Scope) -> Result<(), AppError> {
    if caller.may(scope) {
        return Ok(());
    }
    Err(AppError::of(
        ErrorCode::Forbidden,
        format!(
            "That API key was not issued with the \"{}\" scope. \
             Issue a new key with it from Settings.",
            scope.as_str()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use afisharr_core::api_keys::ScopeSet;

    use super::*;
    use crate::authentication::Credential;

    fn key(scopes: ScopeSet) -> Authenticated {
        Authenticated {
            user_id: "U".to_owned(),
            is_admin: true,
            credential: Credential::ApiKey {
                id: "K".to_owned(),
                scopes,
            },
        }
    }

    #[test]
    fn every_requirement_names_a_distinct_scope() {
        // Two markers on one scope is one route silently granting another's
        // capability, and it reads correctly at both call sites.
        let named = [
            FilesRead::SCOPE,
            EventsRead::SCOPE,
            SessionsManage::SCOPE,
            AccountManage::SCOPE,
            KeysManage::SCOPE,
            PlexRead::SCOPE,
        ];
        let mut sorted = named.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), named.len());
        assert_eq!(sorted.len(), Scope::ALL.len(), "a scope no route asks for");
    }

    #[test]
    fn a_key_without_the_scope_is_refused_and_told_which_one() {
        let refusal = permit(&key(ScopeSet::of([Scope::FilesRead])), Scope::KeysManage)
            .expect_err("a key issued to read files must not manage keys");
        assert_eq!(refusal.problem().code, ErrorCode::Forbidden);
        assert!(
            refusal.problem().message.contains("keys:manage"),
            "the scope to re-issue with must be named: {}",
            refusal.problem().message
        );
    }

    #[test]
    fn a_key_holding_the_scope_is_admitted() {
        assert!(permit(&key(ScopeSet::of([Scope::FilesRead])), Scope::FilesRead).is_ok());
    }

    #[test]
    fn a_scope_does_not_stand_in_for_administrator_rights() {
        // The ceiling is both, and neither substitutes for the other: a key
        // holding every scope, for an account that does not administer this
        // instance, reaches nothing in `Administrator`.
        let mut ordinary = key(ScopeSet::of(Scope::ALL));
        ordinary.is_admin = false;
        assert!(permit(&ordinary, Scope::KeysManage).is_ok());
        assert!(!ordinary.is_admin, "the rights check is the other half");
    }
}
