// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading the settings document a first start seeds `settings` with.

use std::path::Path;

use afisharr_core::settings::SettingsBody;
use anyhow::{Context, Result, bail};

/// Prefix for the environment overrides this loader understands.
const ENV_PREFIX: &str = "AFISHARR_";

/// The settings document in the two readings a start needs.
///
/// One document was not enough, and the gap was not cosmetic: a first start
/// *persists* what it was handed, so handing it the document with
/// [`apply_deployment_environment`] already folded in wrote `AFISHARR_PORT`,
/// `AFISHARR_BIND_ADDRESS`, `AFISHARR_TRUST_PROXY` and `AFISHARR_PUBLIC_ORIGIN`
/// into `settings` on the day the container first booted. Those four are laid
/// back over the stored row on every later start, and only where the variable
/// *is* set — so a value seeded on day one could never be un-set afterwards. An
/// operator who copied `AFISHARR_TRUST_PROXY=10.0.0.0/8` from a template, later
/// deleted the line, and restarted still trusted the whole `/8`, with no route
/// on any surface that edits it and nothing on screen saying so (`I-SEC-1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuration {
    /// What a first start writes into `settings`: the file, plus the three
    /// instance fields the environment seeds.
    pub seed: SettingsBody,
    /// What this start runs with: [`Self::seed`] with the deployment
    /// environment laid over it, in memory and nowhere else.
    pub effective: SettingsBody,
}

impl From<SettingsBody> for Configuration {
    /// A document that states itself, for a caller with no environment behind
    /// it — the tests, which set no `AFISHARR_*` variable and would have to
    /// build the same value twice.
    fn from(body: SettingsBody) -> Self {
        Self {
            seed: body.clone(),
            effective: body,
        }
    }
}

/// Builds the settings document from the optional config file and the
/// environment, in that order.
///
/// The *file* is read only by a first start. Once `settings` holds a row it is
/// the source of truth for everything the operator edits through the interface,
/// because a config file that silently overrode those edits on the next restart
/// would make the settings page lie about the running configuration.
///
/// The *environment* is not the same thing and is not treated as one. A
/// variable in a compose file is a statement the operator makes on every single
/// start, and the container is where they make it: `AFISHARR_PUBLIC_ORIGIN`,
/// `AFISHARR_TRUST_PROXY`, `AFISHARR_BIND_ADDRESS` and `AFISHARR_PORT` describe
/// how this deployment is reached, which is not something the instance can
/// learn from a row written the first time it ever booted. So
/// [`apply_deployment_environment`] runs again over the stored document on
/// every start (`startup::sequence`), it is kept out of
/// [`Configuration::seed`] so that no start ever writes it back, and a settings
/// surface offering these fields has to show which of them the environment is
/// holding.
///
/// The other values read on every start are read that way for ordering rather
/// than for intent: `logging`, because the log is opened before the database,
/// and `backup.retainedPreMigration`, because the pre-migration prune runs
/// before the row is loaded.
///
/// # Errors
/// Returns an error naming the file when it cannot be read, or naming the field
/// when it holds something the typed document rejects.
pub fn load(config_file: &Path) -> Result<Configuration> {
    let mut body = match std::fs::read_to_string(config_file) {
        Ok(text) => {
            toml::from_str(&text).with_context(|| format!("reading {}", config_file.display()))?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => SettingsBody::default(),
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", config_file.display()));
        }
    };

    apply_seed_environment(&mut body)?;
    // Taken before the deployment overlay rather than after it, because this is
    // the copy a first start persists. The overlay describes the *current*
    // deployment and is re-applied on every start from the environment itself,
    // so writing it into the row would make it permanent.
    let seed = body.clone();
    apply_deployment_environment(&mut body)?;
    Ok(Configuration {
        seed,
        effective: body,
    })
}

/// Applies the handful of settings a container operator seeds without a file.
///
/// Deliberately a short, named list rather than a generic path-to-field mapper:
/// the settings document rejects unknown fields, and a generic mapper would
/// reintroduce the partial, unvalidated writes that PRD §19.5 rejects a
/// key-value settings table for.
///
/// Called by [`load`] alone, and therefore only over the document a *first*
/// start seeds `settings` with. These three are seeds and not standing
/// statements, which is why they are not in [`apply_deployment_environment`]:
/// `startup::sequence` writes `instance.timezone`, `instance.locale` and
/// `instance.device_name` into a persisted row on every start, so re-applying
/// them over the stored settings turned a compose variable into a silent edit
/// of the operator's saved document — an `AFISHARR_TIMEZONE=UTC` left in a
/// template reverted their saved `Europe/Copenhagen` at every restart, and the
/// instance renamed itself in their plex.tv device list while it was there.
/// After the first start those fields are the operator's, and they change them
/// where they can see them.
///
/// # Errors
/// Returns an error naming the variable when it is set to nothing, to something
/// that is not text, or to a value the field's type rejects.
fn apply_seed_environment(body: &mut SettingsBody) -> Result<()> {
    if let Some(value) = override_value("TIMEZONE")? {
        body.instance.timezone = value;
    }
    if let Some(value) = override_value("LOCALE")? {
        body.instance.locale = value;
    }
    if let Some(value) = override_value("DEVICE_NAME")? {
        body.instance.device_name = value;
    }
    Ok(())
}

/// The overrides that describe how *this* deployment is reached.
///
/// The subset `startup::sequence` lays back over the stored row on every start,
/// and the reason the two functions are not one: these four say where the
/// container is and how it is fronted, which is not something a row written the
/// day it first booted can know. An operator states them in their compose file
/// on every single start, so an instance that returned the row verbatim made
/// `AFISHARR_PUBLIC_ORIGIN` dead on every instance that had started once.
///
/// None of them is written back anywhere, and that is enforced by
/// [`Configuration`] rather than promised: the copy a first start persists is
/// taken *before* this runs. A variable folded into the seeded row would be
/// permanent, because the overlay on every later start only assigns where the
/// variable is still set — so removing it from the compose file could never
/// remove it from the instance.
///
/// `logging.level` rides with them because it is read the same way — before the
/// database is open — and is likewise never persisted.
///
/// A settings surface offering any of these fields has to show which of them
/// the environment is currently holding.
///
/// # Errors
/// As [`apply_seed_environment`].
pub fn apply_deployment_environment(body: &mut SettingsBody) -> Result<()> {
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
    if let Some(value) = override_value("PUBLIC_ORIGIN")? {
        // Absolute, and judged when the router is built rather than here: this
        // loader's job is to read the document, and a URL that will not parse
        // is reported by the one place that has to use it.
        body.http.public_origin = Some(value);
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
        let configuration = load(&dir.path().join("afisharr.toml")).unwrap();
        assert_eq!(configuration.effective, SettingsBody::default());
        assert_eq!(configuration.seed, SettingsBody::default());
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

        let body = load(&file).unwrap().effective;
        assert_eq!(body.http.port, 9123);
        assert_eq!(body.http.trust_proxy, vec!["10.0.0.0/8".to_owned()]);
    }

    #[test]
    fn what_a_first_start_persists_still_carries_the_config_file() {
        // The bound on withholding the deployment environment from the seed:
        // the *file* is read by a first start and by nothing afterwards, so a
        // `seed` that dropped these fields would lose the operator's stated
        // port and trusted list the moment the row was written.
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("afisharr.toml");
        std::fs::write(
            &file,
            "[http]\nport = 9123\ntrustProxy = [\"10.0.0.0/8\"]\n",
        )
        .unwrap();

        let seed = load(&file).unwrap().seed;
        assert_eq!(seed.http.port, 9123);
        assert_eq!(seed.http.trust_proxy, vec!["10.0.0.0/8".to_owned()]);
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
