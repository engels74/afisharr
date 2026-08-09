// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Argon2id, at the one parameter set this product hashes with.

use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};

use crate::{accounts::AccountError, entropy};

/// The Argon2id cost this instance hashes at.
///
/// PRD §21.4.2 asks for roughly 250 ms on the reference machine of §21.1 — four
/// x86-64 cores, circa 2020 — and `m = 64 MiB, t = 2, p = 1` is the point that
/// lands there with a single lane. Lanes are held at one deliberately: the
/// surface is admin-only (D-007) with at most four concurrent operators, so
/// spending the budget on memory hardness rather than on parallelism costs
/// nothing here and costs an attacker with a GPU a great deal.
///
/// Raising these is a change to every stored hash's verification cost, not to
/// the hashes themselves — a PHC string carries its own parameters, so an
/// account hashed under an older set keeps verifying.
pub const PARAMETERS: Cost = Cost {
    memory_kib: 64 * 1024,
    iterations: 2,
    lanes: 1,
};

/// One Argon2id cost setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cost {
    /// Memory in kibibytes.
    pub memory_kib: u32,
    /// Passes over that memory.
    pub iterations: u32,
    /// Degree of parallelism.
    pub lanes: u32,
}

/// Hashes `plaintext` into a PHC string, on a blocking thread.
///
/// The work is deliberately a quarter of a second, which is a quarter of a
/// second a Tokio worker must not spend, so the call is not offered in a form
/// that lets a handler forget to move it (§24.2.4).
///
/// # Errors
/// Returns [`AccountError::Hashing`] when the parameters are rejected or the
/// hash cannot be produced, and [`AccountError::Interrupted`] when the blocking
/// task did not complete.
pub async fn hash(plaintext: String) -> Result<String, AccountError> {
    tokio::task::spawn_blocking(move || hash_blocking(&plaintext))
        .await
        .map_err(|_| AccountError::Interrupted)?
}

/// Checks `plaintext` against a stored PHC string, on a blocking thread.
///
/// A stored hash that will not parse is [`AccountError::Hashing`], never
/// `Ok(false)`. "This row's proof is unreadable" and "this password is wrong"
/// are different facts, and collapsing them is failure pattern P1 applied to
/// the one credential that guards everything else.
///
/// # Errors
/// Returns [`AccountError::Hashing`] when the stored string is not a readable
/// PHC hash, and [`AccountError::Interrupted`] when the blocking task did not
/// complete.
pub async fn verify(plaintext: String, phc: String) -> Result<bool, AccountError> {
    tokio::task::spawn_blocking(move || verify_blocking(&plaintext, &phc))
        .await
        .map_err(|_| AccountError::Interrupted)?
}

fn argon2() -> Result<Argon2<'static>, AccountError> {
    let params = Params::new(
        PARAMETERS.memory_kib,
        PARAMETERS.iterations,
        PARAMETERS.lanes,
        None,
    )
    .map_err(|source| AccountError::Hashing(source.to_string()))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// The salt length RFC 9106 recommends, and what `SaltString` encodes.
const SALT_BYTES: usize = 16;

fn hash_blocking(plaintext: &str) -> Result<String, AccountError> {
    // Drawn through the same CSPRNG path as every other secret this crate
    // mints, rather than through the hashing crate's own RNG re-export.
    let salt_bytes = entropy::bytes::<SALT_BYTES>();
    let salt = SaltString::encode_b64(&salt_bytes)
        .map_err(|source| AccountError::Hashing(source.to_string()))?;
    argon2()?
        .hash_password(plaintext.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|source| AccountError::Hashing(source.to_string()))
}

fn verify_blocking(plaintext: &str, phc: &str) -> Result<bool, AccountError> {
    let parsed =
        PasswordHash::new(phc).map_err(|source| AccountError::Hashing(source.to_string()))?;
    // The verifier is built from the stored string's own parameters, so an
    // account hashed before a cost change still signs in.
    Ok(Argon2::default()
        .verify_password(plaintext.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_password_verifies_against_its_own_hash() {
        let phc = hash("correct horse battery staple".to_owned())
            .await
            .expect("hashing must succeed");
        assert!(
            verify("correct horse battery staple".to_owned(), phc)
                .await
                .expect("verification must succeed")
        );
    }

    #[tokio::test]
    async fn a_different_password_does_not_verify() {
        let phc = hash("correct horse battery staple".to_owned())
            .await
            .expect("hashing must succeed");
        assert!(
            !verify("Correct horse battery staple".to_owned(), phc)
                .await
                .expect("verification must succeed")
        );
    }

    #[tokio::test]
    async fn the_stored_string_names_argon2id_and_its_parameters() {
        let phc = hash("secret".to_owned())
            .await
            .expect("hashing must succeed");
        assert!(phc.starts_with("$argon2id$"), "{phc}");
        assert!(phc.contains("m=65536"), "{phc}");
        assert!(phc.contains("t=2"), "{phc}");
        assert!(phc.contains("p=1"), "{phc}");
    }

    #[tokio::test]
    async fn two_hashes_of_one_password_differ_because_the_salt_does() {
        let first = hash("secret".to_owned())
            .await
            .expect("hashing must succeed");
        let second = hash("secret".to_owned())
            .await
            .expect("hashing must succeed");
        assert_ne!(first, second);
    }

    #[tokio::test]
    async fn an_unreadable_stored_hash_is_an_error_rather_than_a_wrong_password() {
        let outcome = verify("secret".to_owned(), "not-a-phc-string".to_owned()).await;
        assert!(
            matches!(outcome, Err(AccountError::Hashing(_))),
            "expected a hashing error, got {outcome:?}"
        );
    }
}
