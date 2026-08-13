// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The digest form a `Content-Security-Policy` admits a script by.

use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};

/// The `sha256-<base64>` source expression for `script`.
///
/// CSP hashes the script element's exact text content, standard base64 with
/// padding — not the hex every `_hash` column in this product stores, which is
/// why this is its own function rather than a formatting of [`super::hex`].
///
/// ```
/// use afisharr_core::digest;
///
/// assert_eq!(
///     digest::csp_source(""),
///     "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU="
/// );
/// ```
#[must_use]
pub fn csp_source(script: &str) -> String {
    format!(
        "sha256-{}",
        STANDARD.encode(Sha256::digest(script.as_bytes()))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_source_expression_is_prefixed_the_way_csp_expects() {
        let source = csp_source("console.log(1)");
        assert!(source.starts_with("sha256-"), "{source}");
    }

    #[test]
    fn the_encoding_is_padded_base64_rather_than_hex() {
        let source = csp_source("console.log(1)");
        let encoded = source.trim_start_matches("sha256-");
        assert_eq!(encoded.len(), 44, "{encoded}");
        assert!(encoded.ends_with('='), "{encoded}");
    }

    #[test]
    fn one_changed_byte_changes_the_source_expression() {
        assert_ne!(csp_source("a"), csp_source("b"));
    }
}
