// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The one password check on the HTTP surface.
//!
//! Its own module because it has two callers — `authentication::password_login`
//! and `setup::recover_routes` — and a rule about what makes a password
//! acceptable that is stated twice is one statement nobody tested (P7).

use afisharr_core::accounts::{self, User};

use crate::error::{AppError, AppResult};

/// A PHC string no password verifies against.
///
/// Verified against when the username is unknown, so an unknown account costs
/// the same quarter-second as a wrong password. Without it, sign-in is a
/// username oracle that answers in single-digit milliseconds.
pub(crate) const ABSENT_ACCOUNT_HASH: &str = "$argon2id$v=19$m=65536,t=2,p=1$\
    b+hBjerprIEAZe5xVF9rvQ$eOa9cVGup14UK8k8/VOkO5D8I/fsVNg/ejjps/+PC8E";

/// Verifies a password against an account that may not exist.
///
/// The dummy hash is spent for its cost and never for its answer. An account
/// with no `password_hash` is not an account whose password is unknown: the
/// schema's `CHECK ((kind = 'Local') = (password_hash IS NOT NULL))` makes
/// every Plex-linked row one, and such an account signs in through the pin
/// exchange and nowhere else. Returning the comparison's own result there made
/// [`ABSENT_ACCOUNT_HASH`] a live credential for every one of them — a constant
/// published in this source file, standing between an anonymous caller and a
/// session as any linked viewer, with nothing but its unguessed preimage
/// holding the door. Cost is what the dummy is for; the answer is `false` the
/// moment there was no stored hash to answer about (P2).
///
/// # Errors
/// Returns an internal failure when a stored hash will not parse. A corrupt row
/// is not a wrong password, and reporting it as one sends the operator to reset
/// a credential that is fine.
pub(crate) async fn verify_password(user: Option<&User>, password: String) -> AppResult<bool> {
    verify_against(user, password, ABSENT_ACCOUNT_HASH).await
}

/// [`verify_password`], with the stand-in hash named.
///
/// A parameter so the rule above is testable: the guarantee is that a
/// password-less account is refused *even when the presented password verifies
/// against the stand-in*, and a test cannot demonstrate that against a constant
/// whose preimage nobody has.
async fn verify_against(user: Option<&User>, password: String, absent: &str) -> AppResult<bool> {
    let stored = user.and_then(|user| user.password_hash.clone());
    let has_password = stored.is_some();
    let phc = stored.unwrap_or_else(|| absent.to_owned());
    match accounts::verify(password, phc).await {
        Ok(accepted) => Ok(has_password && accepted),
        Err(error) => Err(AppError::internal(error)),
    }
}

#[cfg(test)]
mod tests {
    use afisharr_core::{accounts::UserKind, time::Timestamp};

    use super::*;

    /// An account of `kind` holding `password_hash`.
    fn account(kind: UserKind, password_hash: Option<String>) -> User {
        User {
            id: "U".to_owned(),
            kind,
            username: "operator".to_owned(),
            email: None,
            display_name: None,
            password_hash,
            plex_account_id: None,
            plex_uuid: None,
            avatar_url: None,
            is_admin: true,
            created_at: Timestamp::EPOCH,
            updated_at: Timestamp::EPOCH,
            last_login_at: None,
            disabled_at: None,
        }
    }

    #[tokio::test]
    async fn an_unknown_account_verifies_against_the_absent_hash_and_fails() {
        assert!(
            !verify_password(None, "anything".to_owned())
                .await
                .expect("the absent hash must parse")
        );
    }

    #[tokio::test]
    async fn an_account_with_no_password_is_refused_even_by_the_stand_in_hash() {
        // The escalation this closes. A Plex-linked row carries
        // `password_hash = NULL`, and comparing the presented password against
        // the stand-in and *returning that answer* made the stand-in a live
        // credential for every one of them: anybody holding its preimage signs
        // in as any linked viewer, by username, with no Plex exchange at all.
        // The preimage of the real constant is nobody's, which is why the
        // stand-in is a parameter here — the rule has to hold when it is known.
        let known = accounts::hash("open sesame please".to_owned())
            .await
            .expect("the test hash must be produced");

        let plex = account(UserKind::Plex, None);
        assert!(
            !verify_against(Some(&plex), "open sesame please".to_owned(), &known)
                .await
                .expect("the stand-in must parse"),
            "an account with no password must never be signed in by the stand-in"
        );

        // And the bound: a local account still signs in with its own password.
        let local = account(UserKind::Local, Some(known.clone()));
        assert!(
            verify_against(Some(&local), "open sesame please".to_owned(), &known)
                .await
                .expect("the stored hash must parse")
        );
    }

    #[tokio::test]
    async fn the_absent_account_hash_is_a_readable_phc_string() {
        // If this string ever stops parsing, every unknown-username sign-in
        // becomes a 500 and the timing oracle it exists to close reopens.
        assert!(
            accounts::verify("x".to_owned(), ABSENT_ACCOUNT_HASH.to_owned())
                .await
                .is_ok()
        );
    }
}
