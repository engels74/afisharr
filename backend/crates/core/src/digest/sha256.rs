// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! SHA-256, rendered the way every `_hash` column stores it.

use sha2::{Digest, Sha256};

/// The lowercase hex SHA-256 of `bytes`.
///
/// ```
/// use afisharr_core::digest;
///
/// assert_eq!(
///     digest::hex(b""),
///     "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
/// );
/// ```
#[must_use]
pub fn hex(bytes: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(bytes.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_digest_is_64_lowercase_hex_characters() {
        let digest = hex("afisharr");
        assert_eq!(digest.len(), 64);
        assert!(
            digest
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn different_inputs_produce_different_digests() {
        assert_ne!(hex("a"), hex("b"));
    }
}
