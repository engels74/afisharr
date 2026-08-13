// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The value that goes in the cookie, and the digest that goes in the table.

use crate::{digest, entropy};

/// How many bytes of OS entropy back one session identifier.
const TOKEN_BYTES: usize = 32;

/// A freshly minted session value.
///
/// The plaintext exists in exactly two places: this struct, on its way into a
/// `Set-Cookie` header, and the operator's browser. The struct is not `Clone`
/// and not `Debug`, so it cannot be copied into a log line by reflex — the
/// digest is what every other part of the product handles.
pub struct SessionToken {
    value: String,
    digest: String,
}

impl SessionToken {
    /// Mints a token from the OS CSPRNG.
    #[must_use]
    pub fn generate() -> Self {
        Self::from_value(hex::encode(entropy::bytes::<TOKEN_BYTES>()))
    }

    /// Wraps a value presented by a caller, so it can be looked up by digest.
    #[must_use]
    pub fn from_value(value: String) -> Self {
        let digest = digest::hex(value.as_bytes());
        Self { value, digest }
    }

    /// The value the browser holds.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The SHA-256 stored in `sessions.id`.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_generated_token_is_64_hex_characters() {
        let token = SessionToken::generate();
        assert_eq!(token.value().len(), TOKEN_BYTES * 2);
        assert!(token.value().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn two_generated_tokens_differ() {
        assert_ne!(
            SessionToken::generate().value(),
            SessionToken::generate().value()
        );
    }

    #[test]
    fn the_digest_is_not_the_value() {
        let token = SessionToken::generate();
        assert_ne!(token.digest(), token.value());
    }

    #[test]
    fn a_presented_value_hashes_to_the_digest_it_was_stored_under() {
        let minted = SessionToken::generate();
        let presented = SessionToken::from_value(minted.value().to_owned());
        assert_eq!(minted.digest(), presented.digest());
    }
}
