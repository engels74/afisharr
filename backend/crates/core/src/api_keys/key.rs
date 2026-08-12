// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The key an operator copies once, and the record that outlives it.

use crate::{api_keys::ScopeSet, digest, entropy, time::Timestamp};

/// How many leading characters of a key are stored in the clear.
///
/// Eight is enough to tell two keys apart in a list and far too few to guess
/// the remaining fifty-six from (PRD §19.6).
pub const PREFIX_LENGTH: usize = 8;

/// How many bytes of OS entropy back one key.
const KEY_BYTES: usize = 32;

/// A key at the one moment its plaintext exists.
///
/// Not `Clone` and not `Debug`: the plaintext is shown once, in one response,
/// and is unrecoverable afterwards. A derived `Debug` would put it in the first
/// log line that formats the struct.
pub struct IssuedApiKey {
    value: String,
    digest: String,
    prefix: String,
}

impl IssuedApiKey {
    /// Mints a key from the OS CSPRNG.
    #[must_use]
    pub fn generate() -> Self {
        Self::from_value(hex::encode(entropy::bytes::<KEY_BYTES>()))
    }

    /// Wraps a value presented by a caller, so it can be looked up by digest.
    #[must_use]
    pub fn from_value(value: String) -> Self {
        let digest = digest::hex(value.as_bytes());
        let prefix = value.chars().take(PREFIX_LENGTH).collect();
        Self {
            value,
            digest,
            prefix,
        }
    }

    /// The plaintext, shown once at creation.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The SHA-256 stored in `api_keys.key_hash`.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// The leading characters stored in `api_keys.prefix`.
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
}

/// One row of `api_keys`, as the interface lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyRecord {
    /// ULID primary key.
    pub id: String,
    /// The name the operator gave the key.
    pub name: String,
    /// The leading characters, for display.
    pub prefix: String,
    /// What this key may reach.
    ///
    /// Chosen when the key was issued and never widened afterwards: a key that
    /// needs another capability is a new key, so that the credential an
    /// integration already holds cannot grow while it holds it.
    pub scopes: ScopeSet,
    /// When the key was issued.
    pub created_at: Timestamp,
    /// The account that issued it, if that account still exists.
    pub created_by: Option<String>,
    /// When the key was last accepted on a request.
    pub last_used_at: Option<Timestamp>,
    /// When the key was revoked, if it was.
    pub revoked_at: Option<Timestamp>,
}

impl ApiKeyRecord {
    /// Whether this key may still authenticate a request.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.revoked_at.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_generated_key_is_64_hex_characters() {
        let key = IssuedApiKey::generate();
        assert_eq!(key.value().len(), KEY_BYTES * 2);
        assert!(key.value().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn the_prefix_is_the_first_eight_characters_of_the_value() {
        let key = IssuedApiKey::generate();
        assert_eq!(key.prefix(), &key.value()[..PREFIX_LENGTH]);
    }

    #[test]
    fn the_stored_digest_is_not_the_value_and_does_not_contain_it() {
        let key = IssuedApiKey::generate();
        assert_ne!(key.digest(), key.value());
        assert!(!key.digest().contains(key.value()));
    }

    #[test]
    fn a_presented_value_hashes_to_the_digest_it_was_stored_under() {
        let issued = IssuedApiKey::generate();
        let presented = IssuedApiKey::from_value(issued.value().to_owned());
        assert_eq!(issued.digest(), presented.digest());
    }
}
