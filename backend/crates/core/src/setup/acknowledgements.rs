// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The two steps that complete by acknowledgement, and the one that ends setup.

use sqlx::SqliteConnection;

use crate::{leases::LeaseName, storage::WriteOperation, time::Timestamp};

/// The acknowledgement the packs step writes.
///
/// Choosing no starter packs is a valid choice, so the step cannot be detected
/// by looking for an installed pack (PRD §7.14).
pub const PACKS_ACK: &str = "packs";

/// The acknowledgement the report step writes.
///
/// The report writes nothing by design (D-026), so there is no other trace of
/// having read it.
pub const REPORT_ACK: &str = "existingCollections";

/// Records that an acknowledgement-only step was completed.
///
/// The list is rewritten rather than appended to, and the write is idempotent:
/// acknowledging twice leaves one entry, which is what makes a retried request
/// harmless.
#[derive(Debug)]
pub struct AckSetupStep {
    /// The acknowledgement to record, one of [`PACKS_ACK`] or [`REPORT_ACK`].
    pub acknowledgement: &'static str,
    /// The instant of the write.
    pub at: Timestamp,
}

impl WriteOperation for AckSetupStep {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let stored: String =
            sqlx::query_scalar!("SELECT setup_acked_steps FROM instance WHERE id = 1")
                .fetch_one(&mut *conn)
                .await?;
        let mut acked: Vec<String> = serde_json::from_str(&stored).unwrap_or_default();
        if acked.iter().any(|step| step == self.acknowledgement) {
            return Ok(());
        }
        acked.push(self.acknowledgement.to_owned());

        let encoded = serde_json::to_string(&acked)
            .map_err(|source| sqlx::Error::Encode(Box::new(source)))?;
        let at = self.at.as_millis();
        sqlx::query!(
            "UPDATE instance SET setup_acked_steps = ?1, updated_at = ?2 WHERE id = 1",
            encoded,
            at
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

/// Ends setup: stamps `instance.setup_completed_at` and drops the claim.
///
/// The in-memory token and the browser cookie are cleared by the caller, which
/// owns both. This operation owns the two facts that live in the database, and
/// it writes them together so an instance can never be complete with a claim
/// still held.
#[derive(Debug)]
pub struct CompleteSetup {
    /// The instant setup finished.
    pub at: Timestamp,
}

impl WriteOperation for CompleteSetup {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let at = self.at.as_millis();
        let mut transaction = sqlx::Connection::begin(conn).await?;
        sqlx::query!(
            "UPDATE instance SET setup_completed_at = ?1, updated_at = ?1 WHERE id = 1",
            at
        )
        .execute(&mut *transaction)
        .await?;
        // Named through `LeaseName` rather than spelled out, like every other
        // path that touches this row (`setup::claim`). A literal here is a
        // second copy of a wire string this crate owns: renaming the lease
        // would update the enum and every reader, compile clean, and leave this
        // DELETE matching nothing — an instance complete with its claim still
        // held, which is the one state the guarantee above rules out.
        let claim = LeaseName::SetupClaim.as_text();
        sqlx::query!("DELETE FROM leases WHERE name = ?1", claim)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await
    }
}
