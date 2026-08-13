// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A whole instance in a scratch directory.

use afisharr::{configuration::DataPaths, startup};
use afisharr_core::{
    settings::{SaveSettings, SettingsBody},
    time::{Clock, SystemClock},
};
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
        self.boot_with(SettingsBody::default()).await
    }

    /// Runs the full boot sequence with a settings document of the test's own.
    ///
    /// The document is re-stated after the boot rather than only handed to it,
    /// because `startup::ensure_settings` seeds `settings` on a *first* start
    /// and returns the stored row on every later one. In production that is the
    /// point — the row is what the operator saved. Here `configured` is what
    /// the test says the instance is, and a second boot over the same directory
    /// dropped it on the floor: `RunningInstance` bound a fresh ephemeral port,
    /// set `publicOrigin` to it, and then served with the *previous* run's dead
    /// port as its origin. Nothing failed at the boot; the next test to reboot
    /// an instance and exercise a flow that reads the origin would have been
    /// answered `400` or `403` naming a port nothing was listening on.
    pub async fn boot_with(&self, configured: SettingsBody) -> startup::Booted {
        let mut booted = startup::boot(&self.paths, configured.clone().into())
            .await
            .expect("a fresh instance must boot");

        if booted.settings.body != configured {
            booted.settings = booted
                .database
                .writer()
                .submit(SaveSettings {
                    body: configured,
                    actor: None,
                    at: SystemClock.now(),
                })
                .await
                .expect("re-stating the test's settings document must succeed");
        }
        booted
    }
}
