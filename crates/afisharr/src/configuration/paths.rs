// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data directory's layout.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

/// The variable that moves the data directory, as a container mount does.
pub const DATA_DIR_ENV_VAR: &str = "AFISHARR_DATA_DIR";

/// Every path the instance writes to, derived from one root.
///
/// One root rather than a path per concern: the backup unit is the database
/// plus its assets plus, if the operator opts in, the key (PRD §21.6.1), and a
/// layout spread across four configurable roots is one an operator cannot back
/// up correctly by accident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPaths {
    root: PathBuf,
}

impl DataPaths {
    /// The layout under `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The layout named by `AFISHARR_DATA_DIR`, defaulting to `./data`.
    ///
    /// # Errors
    /// Returns an error when the variable is set to nothing. That is what a
    /// compose file writes when the value it meant to interpolate was missing,
    /// and taking it would resolve the database, the instance key, and the
    /// backups against the working directory — outside the mount, and outside
    /// the unit an operator backs up (PRD §21.6.1).
    pub fn from_env() -> Result<Self> {
        let Some(configured) = std::env::var_os(DATA_DIR_ENV_VAR) else {
            return Ok(Self::new("data"));
        };
        if configured.to_string_lossy().trim().is_empty() {
            bail!("{DATA_DIR_ENV_VAR} is set to nothing; unset it to use ./data");
        }
        Ok(Self::new(configured))
    }

    /// The data directory itself.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The `SQLite` database.
    #[must_use]
    pub fn database(&self) -> PathBuf {
        self.root.join("afisharr.db")
    }

    /// The instance key, beside the database (D-032).
    #[must_use]
    pub fn secret_key(&self) -> PathBuf {
        self.root.join("secrets.key")
    }

    /// Where automatic pre-migration copies are written.
    #[must_use]
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }

    /// The rotated application log's directory.
    #[must_use]
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// The optional TOML configuration file.
    #[must_use]
    pub fn config_file(&self) -> PathBuf {
        self.root.join("afisharr.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_path_hangs_off_the_one_root() {
        let paths = DataPaths::new("/srv/afisharr");
        for path in [
            paths.database(),
            paths.secret_key(),
            paths.backups(),
            paths.logs(),
            paths.config_file(),
        ] {
            assert!(
                path.starts_with("/srv/afisharr"),
                "{} escaped the root",
                path.display()
            );
        }
    }

    #[test]
    fn the_key_sits_beside_the_database_not_inside_a_subdirectory() {
        let paths = DataPaths::new("/srv/afisharr");
        assert_eq!(paths.secret_key().parent(), paths.database().parent());
    }
}
