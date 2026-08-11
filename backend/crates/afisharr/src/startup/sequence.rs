// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The boot sequence itself.

use std::sync::Arc;

use afisharr_core::{
    backup,
    identifier::Id,
    instance::{EnsureInstance, Instance, NewInstance},
    integrity,
    secrets::{self, SecretKey},
    settings::{SaveSettings, Settings, SettingsBody},
    storage::Database,
    time::{Clock, SystemClock},
};
use anyhow::{Context, Result, bail};
use tracing::{info, warn};

use crate::{
    configuration::DataPaths,
    startup::{
        migrations::{self, RunMigrations},
        reconcile,
    },
};

/// Everything a started instance holds.
pub struct Booted {
    /// The open database.
    ///
    /// Behind an `Arc` because the HTTP surface holds one too, and the write
    /// actor inside it must be the same actor (D-024) rather than a second one
    /// opened alongside.
    pub database: Arc<Database>,
    /// This installation's identity.
    pub instance: Instance,
    /// The effective settings document.
    pub settings: Settings,
    /// The key that seals every credential.
    ///
    /// Behind an `Arc` because `SecretKey` is deliberately not `Clone` and the
    /// HTTP surface needs the same key, not a copy of it.
    pub secret_key: Arc<SecretKey>,
}

impl std::fmt::Debug for Booted {
    /// Prints everything except the key.
    ///
    /// `SecretKey` is not `Debug` on purpose, and a container that derived
    /// `Debug` around it would put the instance key into the first `?booted`
    /// anyone writes.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Booted")
            .field("database", &self.database)
            .field("instance", &self.instance)
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

/// Runs the whole boot sequence and returns the started instance.
///
/// # Errors
/// Returns an error, and does not start, when the schema is newer than this
/// binary knows, when the pre-migration backup cannot be written, or when the
/// post-migration integrity checks report damage.
pub async fn boot(paths: &DataPaths, configured: SettingsBody) -> Result<Booted> {
    let clock = SystemClock;

    let database = Database::open(paths.database())
        .await
        .with_context(|| format!("opening {}", paths.database().display()))?;

    let state = migrations::inspect(database.readers())
        .await
        .context("reading the applied-migration table")?;
    state.ensure_openable()?;

    if state.pending.is_empty() {
        info!(schema_version = state.newest_known, "schema is up to date");
    } else {
        migrate(paths, &database, &state, &configured, &clock).await?;
    }

    let settings = ensure_settings(&database, configured, &clock).await?;
    let instance = ensure_instance(&database, &settings.body, &clock).await?;

    let key_path = paths.secret_key();
    let secret_key = tokio::task::spawn_blocking(move || secrets::load_or_create(&key_path))
        .await
        .context("the secret-key task did not complete")?
        .context("resolving the instance secret key")?;

    reconcile::run(database.writer(), &instance.instance_id, clock.now()).await?;

    info!(
        instance_id = %instance.instance_id,
        app_version = %instance.app_version,
        components = ?crate::observability::components(),
        "afisharr started"
    );

    Ok(Booted {
        database: Arc::new(database),
        instance,
        settings,
        secret_key: Arc::new(secret_key),
    })
}

/// Backs up, migrates, and verifies — in that order, with no step skippable.
async fn migrate(
    paths: &DataPaths,
    database: &Database,
    state: &migrations::MigrationState,
    configured: &SettingsBody,
    clock: &SystemClock,
) -> Result<()> {
    let from_version = state.applied.iter().copied().max().unwrap_or(0);

    // A forward-only migration whose backup failed has no recovery path, which
    // is the trade forward-only makes and only survives if the backup is real.
    // So this is not best-effort: a failure here stops the start.
    let destination = backup::pre_migration_path(paths.backups(), from_version, clock.now());
    let written = backup::copy(database.path(), &destination)
        .await
        .context("taking the pre-migration backup; the migration has not run")?;
    info!(backup = %written.display(), from_version, "pre-migration backup written");

    // The copy written above is named as protected rather than trusted to rank
    // first: `retained_pre_migration` is an operator's number, and a `0` — or a
    // leftover copy of a newer schema outranking this one — would otherwise
    // delete the very backup this forward-only migration stands behind
    // (`I-DATA-8`). Retention says how much history to hold, never whether this
    // migration is protected.
    let keep = usize::from(configured.backup.retained_pre_migration);
    match backup::prune(paths.backups(), keep, &written).await {
        Ok(removed) if !removed.is_empty() => {
            info!(
                removed = removed.len(),
                keep, "pruned older pre-migration backups"
            );
        }
        Ok(_) => {}
        // Pruning is housekeeping. The backup that matters was already written,
        // and refusing to start because an old copy could not be deleted would
        // trade a working upgrade for a full disk warning.
        Err(error) => warn!(%error, "could not prune older pre-migration backups"),
    }

    database
        .writer()
        .submit(RunMigrations)
        .await
        .context("applying pending migrations")?;
    info!(
        applied = state.pending.len(),
        to_version = state.newest_known,
        "migrations applied"
    );

    let report = integrity::verify(database.readers())
        .await
        .context("running foreign_key_check and integrity_check after migrating")?;
    if !report.is_clean() {
        bail!(
            "the database failed its post-migration checks. Broken references: {:?}. \
             Structural problems: {:?}. The pre-migration backup is at {}.",
            report.broken_references,
            report.structural_problems,
            written.display()
        );
    }
    info!("post-migration integrity checks are clean");

    Ok(())
}

/// Seeds `settings` on a first start; afterwards the stored row is the truth,
/// with the environment's deployment variables laid back over it.
///
/// The row alone is not enough, and the gap is not theoretical: `publicOrigin`,
/// `trustProxy`, `bindAddress` and `port` describe how *this* deployment is
/// reached, an operator states them in their compose file, and a row written on
/// the day the container first booted cannot know any of it. Returning the row
/// verbatim makes `AFISHARR_PUBLIC_ORIGIN` dead on every instance that has
/// started once — so the operator who sets it is told to set the very thing
/// they have set, and the hosted Plex sign-in stays unavailable with nothing
/// explaining why.
///
/// Applied in memory rather than written back. The row is what the operator
/// saved, and rewriting it on every boot would turn a compose variable into a
/// silent edit of their saved document. That means a settings surface offering
/// these fields has to show which of them the environment is currently holding
/// — the same obligation `logging` and `backup.retainedPreMigration` already
/// carry (`configuration::load`).
async fn ensure_settings(
    database: &Database,
    configured: SettingsBody,
    clock: &SystemClock,
) -> Result<Settings> {
    if let Some(mut stored) = afisharr_core::settings::load(database.readers())
        .await
        .context("reading settings")?
    {
        crate::configuration::apply_environment(&mut stored.body)
            .context("applying the environment over the stored settings")?;
        return Ok(stored);
    }

    database
        .writer()
        .submit(SaveSettings {
            body: configured,
            actor: None,
            at: clock.now(),
        })
        .await
        .context("seeding settings on first start")
}

/// Writes the instance row, minting its identifiers only on a first start.
async fn ensure_instance(
    database: &Database,
    settings: &SettingsBody,
    clock: &SystemClock,
) -> Result<Instance> {
    database
        .writer()
        .submit(EnsureInstance {
            identity: NewInstance {
                device_name: settings.instance.device_name.clone(),
                timezone: settings.instance.timezone.clone(),
                locale: settings.instance.locale.clone(),
                app_version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            instance_id: Id::generate(clock),
            client_identifier: Id::generate(clock),
            at: clock.now(),
        })
        .await
        .context("writing the instance row")
}
