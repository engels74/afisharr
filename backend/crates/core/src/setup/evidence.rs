// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The facts the resume rule reads, gathered from the database.

use sqlx::SqlitePool;

use crate::setup::{ClaimState, PACKS_ACK, REPORT_ACK};

/// The secret whose presence means Plex is connected.
const PLEX_TOKEN_SECRET: &str = "plex.token";

/// The one integration PRD §7.14 requires before the wizard moves on.
const TMDB_SECRET: &str = "tmdb.apiKey";

/// What the database says about how far setup has got.
///
/// Every field is a fact written by the step it belongs to. None of them is a
/// client assertion, and none is derived from another: an instance that
/// acknowledged packs but lost its Plex connection reports the Plex step, not
/// the packs step.
//
// Seven bools rather than an enum or a bitflag set: they are seven independent
// facts, each read once and by name in `SetupStep::resume_at`, and the lint's
// usual remedy — collapse them into a state enum — is exactly the flattening
// the derived-step rule forbids. The step is computed *from* these; it is not
// one of them.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Evidence {
    /// A live `setup:claim` lease whose owner is this request's cookie.
    pub claim_held_by_caller: bool,
    /// An enabled administrator account exists.
    pub admin_exists: bool,
    /// A `plex_server` row exists and the `plex.token` secret is present.
    pub plex_connected: bool,
    /// At least one library is managed.
    pub library_selected: bool,
    /// The `tmdb.apiKey` secret is present.
    pub tmdb_configured: bool,
    /// `packs` is in `instance.setup_acked_steps`.
    pub packs_acknowledged: bool,
    /// `existingCollections` is in `instance.setup_acked_steps`.
    pub report_acknowledged: bool,
}

/// Reads every fact the resume rule needs.
///
/// Secret presence is a row test, never a decryption: a database restored
/// without its key holds a `plex.token` whose value is unobservable, and
/// reporting that as "Plex is not connected" would send the operator back
/// through a step whose evidence is right there (P1).
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn read(readers: &SqlitePool, claim: ClaimState) -> Result<Evidence, sqlx::Error> {
    let acked: Vec<String> =
        sqlx::query_scalar!("SELECT setup_acked_steps FROM instance WHERE id = 1")
            .fetch_optional(readers)
            .await?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

    Ok(Evidence {
        claim_held_by_caller: matches!(claim, ClaimState::HeldByCaller { .. }),
        admin_exists: crate::accounts::admin_exists(readers).await?,
        plex_connected: plex_server_exists(readers).await?
            && secret_present(readers, PLEX_TOKEN_SECRET).await?,
        library_selected: managed_library_exists(readers).await?,
        tmdb_configured: secret_present(readers, TMDB_SECRET).await?,
        packs_acknowledged: acked.iter().any(|step| step == PACKS_ACK),
        report_acknowledged: acked.iter().any(|step| step == REPORT_ACK),
    })
}

async fn plex_server_exists(readers: &SqlitePool) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query_scalar!("SELECT 1 FROM plex_server WHERE id = 1")
            .fetch_optional(readers)
            .await?
            .is_some(),
    )
}

async fn managed_library_exists(readers: &SqlitePool) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query_scalar!("SELECT 1 FROM libraries WHERE is_managed = 1 LIMIT 1")
            .fetch_optional(readers)
            .await?
            .is_some(),
    )
}

async fn secret_present(readers: &SqlitePool, name: &str) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query_scalar!("SELECT 1 FROM secrets WHERE name = ?1", name)
            .fetch_optional(readers)
            .await?
            .is_some(),
    )
}
