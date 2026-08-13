// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the credential issuing a key is allowed to put in it.
//!
//! Apart from the route because it is the one rule on this surface that is
//! about the *caller* rather than about the request: `requested_scopes` asks
//! whether a name is a scope this instance grants, and this asks whether it is
//! one the caller may pass on. Two questions, and only the second is a ceiling.

use afisharr_core::api_keys::{Scope, ScopeSet};

use crate::{
    authentication::Authenticated,
    error::{AppError, AppResult, ErrorCode, Problem},
};

/// Refuses a scope the calling credential does not itself hold.
///
/// A scope is a ceiling and never a grant, and a ceiling a credential can step
/// over by minting a second credential is not one. Without this, a key issued
/// with `keys:manage` alone — the rotation script's key, the one most likely to
/// sit in an automation's environment — could `POST` itself a replacement
/// holding `files:read`, `events:read`, `sessions:manage`, and `account:manage`,
/// none of which the operator granted it. Revoking the leaked key would not
/// revoke the child, which is the escalation `api_keys::scope` exists to close.
///
/// A session reaches every scope ([`Authenticated::may`]), so an operator
/// sitting at the interface still issues whatever key they like. Only a key
/// issuing a key is held to what it holds.
///
/// # Errors
/// Returns a `forbidden` problem at `/scopes`, naming the scopes that were
/// beyond the caller's reach and the ones that were not.
pub(super) fn within_the_callers_reach(
    caller: &Authenticated,
    scopes: ScopeSet,
) -> AppResult<ScopeSet> {
    let beyond: Vec<&str> = scopes
        .held()
        .into_iter()
        .filter(|scope| !caller.may(*scope))
        .map(Scope::as_str)
        .collect();
    if beyond.is_empty() {
        return Ok(scopes);
    }
    Err(AppError::new(
        Problem::new(
            ErrorCode::Forbidden,
            "A key cannot be issued with a scope the credential issuing it does not hold.",
        )
        .at("/scopes")
        .expecting(reachable(caller), beyond.join(", ")),
    ))
}

/// The scopes the caller may pass on, for a refusal that names them.
fn reachable(caller: &Authenticated) -> String {
    Scope::ALL
        .into_iter()
        .filter(|scope| caller.may(*scope))
        .map(Scope::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
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
    fn a_key_cannot_issue_a_key_wider_than_itself() {
        // The escalation this closes: a rotation script's key, issued with
        // `keys:manage` alone, posting itself a replacement that browses the
        // filesystem and revokes the operator's sessions — capabilities the
        // operator never granted it, on a credential that revoking the first
        // does not revoke.
        let refusal = within_the_callers_reach(
            &key(ScopeSet::of([Scope::KeysManage])),
            ScopeSet::of([Scope::KeysManage, Scope::FilesRead]),
        )
        .expect_err("a key must not widen itself through a second key");
        let problem = refusal.problem();
        assert_eq!(problem.code, ErrorCode::Forbidden);
        assert_eq!(problem.pointer.as_deref(), Some("/scopes"));
        assert_eq!(
            problem
                .mismatch
                .as_ref()
                .map(|mismatch| mismatch.actual.as_str()),
            Some("files:read"),
            "the refusal must name the scope that was beyond reach"
        );
    }

    #[test]
    fn a_key_may_pass_on_the_scopes_it_holds() {
        // The bound: delegating what you were granted is the case
        // `Scope::KeysManage` says an operator may genuinely want.
        let caller = key(ScopeSet::of([Scope::KeysManage, Scope::FilesRead]));
        let asked = ScopeSet::of([Scope::FilesRead]);
        assert_eq!(
            within_the_callers_reach(&caller, asked).expect("within reach"),
            asked
        );
    }

    #[test]
    fn an_operator_at_the_interface_still_issues_whatever_key_they_like() {
        // A session is the account rather than a narrowing of it, so nothing
        // here restricts the operator who is signed in.
        let session = Authenticated {
            user_id: "U".to_owned(),
            is_admin: true,
            credential: Credential::Session {
                digest: "d".to_owned(),
            },
        };
        let every = ScopeSet::of(Scope::ALL);
        assert_eq!(
            within_the_callers_reach(&session, every).expect("a session reaches everything"),
            every
        );
    }
}
