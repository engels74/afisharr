// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading the settings document a first start seeds `settings` with.

use std::path::Path;

use afisharr_core::settings::SettingsBody;
use anyhow::{Context, Result};

/// Prefix for the environment overrides this loader understands.
const ENV_PREFIX: &str = "AFISHARR_";

/// Builds the settings document from the optional config file and the
/// environment, in that order.
///
/// Only a first start uses this. Once `settings` holds a row it is the source
/// of truth, because the operator edits it through the interface and a config
/// file that silently overrode those edits on the next restart would make the
/// settings page lie about the running configuration.
///
/// # Errors
/// Returns an error naming the file when it cannot be read, or naming the field
/// when it holds something the typed document rejects.
pub fn load(config_file: &Path) -> Result<SettingsBody> {
    let mut body = match std::fs::read_to_string(config_file) {
        Ok(text) => {
            toml::from_str(&text).with_context(|| format!("reading {}", config_file.display()))?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => SettingsBody::default(),
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", config_file.display()));
        }
    };

    apply_environment(&mut body)?;
    Ok(body)
}

/// Applies the handful of settings a container operator sets without a file.
///
/// Deliberately a short, named list rather than a generic path-to-field mapper:
/// the settings document rejects unknown fields, and a generic mapper would
/// reintroduce the partial, unvalidated writes that PRD §19.5 rejects a
/// key-value settings table for.
fn apply_environment(body: &mut SettingsBody) -> Result<()> {
    if let Ok(value) = std::env::var(format!("{ENV_PREFIX}TIMEZONE")) {
        body.instance.timezone = value;
    }
    if let Ok(value) = std::env::var(format!("{ENV_PREFIX}LOCALE")) {
        body.instance.locale = value;
    }
    if let Ok(value) = std::env::var(format!("{ENV_PREFIX}DEVICE_NAME")) {
        body.instance.device_name = value;
    }
    if let Ok(value) = std::env::var(format!("{ENV_PREFIX}BIND_ADDRESS")) {
        body.http.bind_address = value;
    }
    if let Ok(value) = std::env::var(format!("{ENV_PREFIX}PORT")) {
        body.http.port = value
            .parse()
            .with_context(|| format!("{ENV_PREFIX}PORT is not a port number"))?;
    }
    if let Ok(value) = std::env::var(format!("{ENV_PREFIX}TRUST_PROXY")) {
        // A list, never a boolean (D-029): comma-separated addresses or CIDRs,
        // and an empty value means nothing is trusted rather than everything.
        body.http.trust_proxy = value
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_owned)
            .collect();
    }
    if let Ok(value) = std::env::var(format!("{ENV_PREFIX}LOG_LEVEL")) {
        body.logging.level = value;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn an_absent_config_file_yields_the_defaults() {
        let dir = TempDir::new().unwrap();
        assert_eq!(
            load(&dir.path().join("afisharr.toml")).unwrap(),
            SettingsBody::default()
        );
    }

    #[test]
    fn a_config_file_is_read_into_the_typed_document() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("afisharr.toml");
        std::fs::write(
            &file,
            "[http]\nport = 9123\ntrustProxy = [\"10.0.0.0/8\"]\n",
        )
        .unwrap();

        let body = load(&file).unwrap();
        assert_eq!(body.http.port, 9123);
        assert_eq!(body.http.trust_proxy, vec!["10.0.0.0/8".to_owned()]);
    }

    #[test]
    fn a_typo_in_the_config_file_is_an_error_naming_the_field() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("afisharr.toml");
        std::fs::write(&file, "[http]\nprot = 9123\n").unwrap();

        let error = format!("{:#}", load(&file).expect_err("a typo must not be ignored"));
        assert!(error.contains("prot"), "{error}");
    }
}
