// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The key beside the database, and the environment override for it.

use std::{fs, io, path::Path};

use crate::secrets::{SecretError, SecretKey};

/// The variable an operator sets to mount the key from a secret manager.
///
/// The environment is the override and never the default: environment variables
/// leak into process listings, crash dumps, and container inspection (D-032).
pub const KEY_ENV_VAR: &str = "AFISHARR_SECRET_KEY";

/// How many bytes a key is.
const KEY_LEN: usize = 32;

/// Mode `0600` — readable and writable by the owner, invisible to anyone else.
#[cfg(unix)]
const KEY_FILE_MODE: u32 = 0o600;

/// Resolves the instance key: the environment override, else the file, else a
/// freshly generated file.
///
/// This is a blocking filesystem call by design. Callers on the Tokio runtime
/// run it through `spawn_blocking`; it happens once, at startup.
///
/// # Errors
/// Returns [`SecretError::KeyEncoding`] when the override is not 64 hex
/// characters, including when it is not text at all; [`SecretError::KeyLength`]
/// when the override or the file holds the wrong number of bytes;
/// [`SecretError::KeyFile`] when the file cannot be read or created; and
/// [`SecretError::Entropy`] when a fresh key cannot be generated.
pub fn load_or_create(path: impl AsRef<Path>) -> Result<SecretKey, SecretError> {
    if let Some(encoded) = std::env::var_os(KEY_ENV_VAR) {
        // `var_os`, because `var` folds "set but not text" into "not set" and
        // this function would then fall through and mint a fresh key. Every
        // stored credential becomes undecryptable the moment the operator fixes
        // their variable, which is the outcome D-032 exists to prevent.
        let encoded = encoded.to_str().ok_or(SecretError::KeyEncoding)?;
        return from_hex(encoded);
    }

    let path = path.as_ref();
    match fs::read(path) {
        Ok(bytes) => {
            let bytes: [u8; KEY_LEN] =
                bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| SecretError::KeyLength {
                        source_name: path.display().to_string(),
                        found: bytes.len(),
                    })?;
            Ok(SecretKey::from_bytes(bytes))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => create(path),
        Err(source) => Err(SecretError::KeyFile {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Writes a fresh key at `path` with owner-only permissions.
fn create(path: &Path) -> Result<SecretKey, SecretError> {
    let key = SecretKey::generate()?;

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| SecretError::KeyFile {
            path: path.to_path_buf(),
            source,
        })?;
    }

    write_owner_only(path, key.as_bytes()).map_err(|source| SecretError::KeyFile {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(key)
}

/// Creates the file with mode `0600` from the outset, never widening it later.
#[cfg(unix)]
fn write_owner_only(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};

    // The mode is set at creation rather than by a chmod afterwards: between
    // create and chmod the key is world-readable, and that window is the
    // vulnerability the mode exists to close.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(KEY_FILE_MODE)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Windows has no mode bits; the file inherits the directory's ACL.
///
/// PRD §21.5 makes `windows/amd64` a best-effort target, and this is one of the
/// places where the platform cannot offer what the primary target does. It is a
/// weaker guarantee, said plainly rather than silently approximated.
#[cfg(not(unix))]
fn write_owner_only(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Decodes the 64 hex characters the environment override carries.
fn from_hex(encoded: &str) -> Result<SecretKey, SecretError> {
    let bytes = hex::decode(encoded.trim()).map_err(|_| SecretError::KeyEncoding)?;
    let bytes: [u8; KEY_LEN] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| SecretError::KeyLength {
            source_name: KEY_ENV_VAR.to_owned(),
            found: bytes.len(),
        })?;
    Ok(SecretKey::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn a_first_start_creates_the_key_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("secrets.key");

        let key = load_or_create(&path).unwrap();
        assert!(path.exists());
        assert_eq!(fs::read(&path).unwrap().len(), KEY_LEN);

        // A second start reads the same key rather than replacing it: a
        // regenerated key makes every stored credential undecryptable.
        let reopened = load_or_create(&path).unwrap();
        let sealed = key.seal(b"plex-token").unwrap();
        assert_eq!(reopened.open("plex.token", &sealed).unwrap(), b"plex-token");
    }

    #[cfg(unix)]
    #[test]
    fn the_key_file_is_created_with_mode_0600() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("secrets.key");
        load_or_create(&path).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, KEY_FILE_MODE, "expected 0600, got {mode:o}");
    }

    #[test]
    fn a_short_key_file_is_reported_rather_than_padded() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("secrets.key");
        fs::write(&path, b"too short").unwrap();

        assert!(matches!(
            load_or_create(&path),
            Err(SecretError::KeyLength { found: 9, .. })
        ));
    }

    #[test]
    fn the_hex_override_decodes_to_the_same_key() {
        let key = SecretKey::generate().unwrap();
        let decoded = from_hex(&hex::encode(key.as_bytes())).unwrap();
        let sealed = key.seal(b"tmdb").unwrap();
        assert_eq!(decoded.open("tmdb.apiKey", &sealed).unwrap(), b"tmdb");
    }

    #[test]
    fn a_malformed_override_is_rejected() {
        assert!(matches!(from_hex("not hex"), Err(SecretError::KeyEncoding)));
        assert!(matches!(
            from_hex("00ff"),
            Err(SecretError::KeyLength { found: 2, .. })
        ));
    }
}
