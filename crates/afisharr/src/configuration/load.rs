// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading the settings document a first start seeds `settings` with.

use std::path::Path;

use afisharr_core::settings::SettingsBody;
use anyhow::{Context, Result, bail};

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
/// The exceptions are the values decided before the row can be read, and they
/// are exceptions of ordering rather than of intent: `logging`, because the log
/// is opened before the database, and `backup.retainedPreMigration`, because
/// the pre-migration prune runs before the row is loaded. Both keep being read
/// from here on every start, and a settings surface offering them has to say so.
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
    if let Some(value) = override_value("TIMEZONE")? {
        body.instance.timezone = value;
    }
    if let Some(value) = override_value("LOCALE")? {
        body.instance.locale = value;
    }
    if let Some(value) = override_value("DEVICE_NAME")? {
        body.instance.device_name = value;
    }
    if let Some(value) = override_value("BIND_ADDRESS")? {
        body.http.bind_address = value;
    }
    if let Some(value) = override_value("PORT")? {
        body.http.port = value
            .parse()
            .with_context(|| format!("{ENV_PREFIX}PORT is not a port number"))?;
    }
    if let Some(value) = override_text("TRUST_PROXY")? {
        // A list, never a boolean (D-029): comma-separated addresses or CIDRs,
        // and an empty value means nothing is trusted rather than everything.
        body.http.trust_proxy = value
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_owned)
            .collect();
    }
    if let Some(value) = override_value("LOG_LEVEL")? {
        body.logging.level = value;
    }
    Ok(())
}

/// The override for `field`, refused when it is set to nothing.
///
/// An empty variable is what a compose file writes when the value it meant to
/// interpolate was not there, and none of these fields has a meaningful empty
/// value: an empty log filter parses and switches logging off entirely, and an
/// empty timezone, locale, device name, or bind address is not one. Taking it
/// silently is the same failure as taking a filter that would not parse.
/// `TRUST_PROXY` reads its own variable because there empty is the deliberate
/// answer — nothing is trusted (D-029).
///
/// # Errors
/// Returns an error naming the variable when it is set to an empty value.
fn override_value(field: &str) -> Result<Option<String>> {
    let Some(value) = override_text(field)? else {
        return Ok(None);
    };
    if value.trim().is_empty() {
        bail!("{ENV_PREFIX}{field} is set to nothing; unset it to keep the configured value");
    }
    Ok(Some(value))
}

/// The value `AFISHARR_<field>` holds, refused when it is not text.
///
/// `std::env::var` reports "not set" and "set to bytes this process cannot
/// read" as the same `Err`, so reading through it discards a value the operator
/// did set. `TRUST_PROXY` reads through here rather than through
/// [`override_value`] because its empty value is a deliberate answer (D-029).
///
/// # Errors
/// Returns an error naming the variable when its value is not valid UTF-8.
fn override_text(field: &str) -> Result<Option<String>> {
    let name = format!("{ENV_PREFIX}{field}");
    let Some(value) = std::env::var_os(&name) else {
        return Ok(None);
    };
    let Some(value) = value.to_str() else {
        bail!("{name} is set to something that is not text");
    };
    Ok(Some(value.to_owned()))
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
