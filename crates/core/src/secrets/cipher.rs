// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! XChaCha20-Poly1305, one random nonce per secret.

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit},
};

use crate::secrets::SecretError;

/// The token written to `secrets.algorithm`.
///
/// Stored per row rather than assumed, so a future key rotation can read old
/// rows instead of guessing at them.
pub const ALGORITHM: &str = "XChaCha20-Poly1305";

/// How many bytes a key is.
const KEY_LEN: usize = 32;

/// How many bytes an `XChaCha20` nonce is.
const NONCE_LEN: usize = 24;

/// The 32-byte key that seals every secret in this instance.
///
/// Deliberately not `Clone`, `Debug`, or `Serialize`: the ways key material
/// escapes are a log line, a copy, and a support bundle.
pub struct SecretKey([u8; KEY_LEN]);

/// A sealed secret: ciphertext, the nonce it was sealed under, and the algorithm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    /// The authenticated ciphertext.
    pub ciphertext: Vec<u8>,
    /// The nonce, unique per secret.
    pub nonce: Vec<u8>,
    /// The algorithm token, so a later binary can tell what it is reading.
    pub algorithm: String,
}

impl SecretKey {
    /// Wraps 32 bytes of key material.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Draws a fresh key from the OS CSPRNG.
    ///
    /// # Errors
    /// Returns [`SecretError::Entropy`] when the OS entropy source is unavailable.
    pub fn generate() -> Result<Self, SecretError> {
        let mut bytes = [0u8; KEY_LEN];
        getrandom::fill(&mut bytes).map_err(|_| SecretError::Entropy)?;
        Ok(Self(bytes))
    }

    /// The raw key material, for writing the key file.
    #[must_use]
    pub(crate) const fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// Seals `plaintext` under a nonce drawn fresh for this secret.
    ///
    /// # Errors
    /// Returns [`SecretError::Entropy`] when the OS entropy source is unavailable.
    pub fn seal(&self, plaintext: &[u8]) -> Result<Sealed, SecretError> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce_bytes).map_err(|_| SecretError::Entropy)?;
        let nonce = XNonce::from(nonce_bytes);

        let ciphertext = XChaCha20Poly1305::new(&self.0.into())
            .encrypt(&nonce, plaintext)
            // The AEAD reports encryption failure without detail; there is no
            // input this construction rejects, so this is unreachable in
            // practice and is still not worth panicking over.
            .map_err(|_| SecretError::Entropy)?;

        Ok(Sealed {
            ciphertext,
            nonce: nonce_bytes.to_vec(),
            algorithm: ALGORITHM.to_owned(),
        })
    }

    /// Opens a sealed secret.
    ///
    /// # Errors
    /// Returns [`SecretError::UnsupportedAlgorithm`] when the row names another
    /// construction, and [`SecretError::Undecryptable`] when the ciphertext does
    /// not authenticate under this key — which is what a database restored
    /// without its key file looks like.
    pub fn open(&self, name: &str, sealed: &Sealed) -> Result<Vec<u8>, SecretError> {
        if sealed.algorithm != ALGORITHM {
            return Err(SecretError::UnsupportedAlgorithm {
                name: name.to_owned(),
                algorithm: sealed.algorithm.clone(),
            });
        }
        let nonce: [u8; NONCE_LEN] =
            sealed
                .nonce
                .as_slice()
                .try_into()
                .map_err(|_| SecretError::Undecryptable {
                    name: name.to_owned(),
                })?;

        XChaCha20Poly1305::new(&self.0.into())
            .decrypt(&XNonce::from(nonce), sealed.ciphertext.as_slice())
            .map_err(|_| SecretError::Undecryptable {
                name: name.to_owned(),
            })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn a_secret_round_trips_through_seal_and_open() {
        let key = SecretKey::generate().unwrap();
        let sealed = key.seal(b"plex-token").unwrap();
        assert_eq!(key.open("plex.token", &sealed).unwrap(), b"plex-token");
    }

    #[test]
    fn each_secret_gets_its_own_nonce() {
        let key = SecretKey::generate().unwrap();
        let first = key.seal(b"same").unwrap();
        let second = key.seal(b"same").unwrap();
        assert_ne!(first.nonce, second.nonce);
        assert_ne!(first.ciphertext, second.ciphertext);
    }

    #[test]
    fn the_algorithm_is_recorded_on_every_sealed_secret() {
        let key = SecretKey::generate().unwrap();
        assert_eq!(key.seal(b"x").unwrap().algorithm, ALGORITHM);
    }

    #[test]
    fn another_key_cannot_open_the_ciphertext() {
        let sealed = SecretKey::generate().unwrap().seal(b"plex-token").unwrap();
        let other = SecretKey::generate().unwrap();
        assert!(matches!(
            other.open("plex.token", &sealed),
            Err(SecretError::Undecryptable { .. })
        ));
    }

    #[test]
    fn a_tampered_ciphertext_fails_authentication() {
        let key = SecretKey::generate().unwrap();
        let mut sealed = key.seal(b"plex-token").unwrap();
        sealed.ciphertext[0] ^= 0xff;
        assert!(matches!(
            key.open("plex.token", &sealed),
            Err(SecretError::Undecryptable { .. })
        ));
    }

    #[test]
    fn an_unknown_algorithm_is_named_rather_than_guessed_at() {
        let key = SecretKey::generate().unwrap();
        let mut sealed = key.seal(b"x").unwrap();
        sealed.algorithm = "AES-GCM".to_owned();
        assert!(matches!(
            key.open("plex.token", &sealed),
            Err(SecretError::UnsupportedAlgorithm { algorithm, .. }) if algorithm == "AES-GCM"
        ));
    }
}
