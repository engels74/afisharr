// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A whole instance in a scratch directory.

use afisharr::{configuration::DataPaths, startup};
use afisharr_core::settings::SettingsBody;
use tempfile::TempDir;

/// A data directory that lives as long as the test.
pub struct TempInstance {
    directory: TempDir,
    paths: DataPaths,
}

impl TempInstance {
    /// A scratch data directory with nothing in it yet.
    pub fn new() -> Self {
        let directory = TempDir::new().expect("a scratch directory");
        let paths = DataPaths::new(directory.path());
        Self { directory, paths }
    }

    /// The layout under the scratch directory.
    pub fn paths(&self) -> &DataPaths {
        &self.paths
    }

    /// Runs the full boot sequence against this directory.
    pub async fn boot(&self) -> startup::Booted {
        startup::boot(&self.paths, SettingsBody::default())
            .await
            .expect("a fresh instance must boot")
    }
}
