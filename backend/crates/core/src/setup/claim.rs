// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `setup:claim` lease, and the cookie that is its other half.

use sqlx::{SqliteConnection, SqlitePool};

use crate::{
    digest,
    leases::{Acquire, Heartbeat, LeaseName, LeaseOwner, held_by},
    storage::WriteOperation,
    time::Timestamp,
};

/// The cookie that carries the claim's other half (PRD §21.4.2).
pub const CLAIM_COOKIE: &str = "afisharr_setup_claim";

/// Ten minutes, sliding on every gated request (PRD §19.6.1).
///
/// Shorter than the token's fifteen on purpose: the claim must expire while the
/// token that created it is still usable. Reverse the two and an operator waits
/// out the claim only to find the token expired while they waited.
pub const CLAIM_TTL_MILLIS: i64 = 10 * 60 * 1000;

/// Who holds the wizard right now, from this request's point of view.
///
/// A claim is active when the lease row is unexpired **and** the request's
/// cookie hashes to its owner. Both halves are required, so a stolen database
/// row proves nothing and a stolen cookie outlives its lease by nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimState {
    /// No live claim exists. The next valid token takes it.
    Unclaimed,
    /// This request's cookie holds the claim.
    HeldByCaller {
        /// When the hold lapses if nothing renews it.
        expires_at: Timestamp,
    },
    /// Another browser holds the claim.
    HeldByAnother {
        /// When the hold lapses, which is the retry time the caller is told.
        expires_at: Timestamp,
    },
}

/// What a claim attempt produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimOutcome {
    /// The caller now holds the claim until the returned instant.
    Granted {
        /// When the hold lapses if nothing renews it.
        expires_at: Timestamp,
    },
    /// Another browser holds it; nothing changed.
    Blocked {
        /// When that hold lapses.
        expires_at: Timestamp,
    },
}

/// Reads the claim's state for a caller presenting `cookie_value`.
///
/// A caller with no cookie passes `None` and is answered
/// [`ClaimState::HeldByAnother`] whenever a live claim exists — which is the
/// truthful answer, and the one that carries the retry time.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn inspect(
    readers: &SqlitePool,
    cookie_value: Option<&str>,
    now: Timestamp,
) -> Result<ClaimState, sqlx::Error> {
    let Some(holder) = held_by(readers, &LeaseName::SetupClaim).await? else {
        return Ok(ClaimState::Unclaimed);
    };
    if now >= holder.expires_at {
        return Ok(ClaimState::Unclaimed);
    }
    let caller = cookie_value.map(owner_of);
    if caller.is_some_and(|caller| caller.as_str() == holder.owner) {
        Ok(ClaimState::HeldByCaller {
            expires_at: holder.expires_at,
        })
    } else {
        Ok(ClaimState::HeldByAnother {
            expires_at: holder.expires_at,
        })
    }
}

/// The lease owner a cookie value hashes to.
///
/// The cookie itself never reaches `leases.owner`, so a database read yields
/// nothing that can be presented as the claim.
fn owner_of(cookie_value: &str) -> LeaseOwner {
    LeaseOwner::token(&digest::hex(cookie_value.as_bytes()))
}

/// Takes the claim for `cookie_value`, or reports who holds it.
///
/// A holder re-presenting their own cookie renews rather than being refused:
/// PRD §19.6.1 orders the claim endpoint holder-first, so a refresh of the
/// claim page costs nothing.
#[derive(Debug)]
pub struct MintClaim {
    /// The cookie value the caller will hold.
    pub cookie_value: String,
    /// The instant of the attempt.
    pub at: Timestamp,
}

impl WriteOperation for MintClaim {
    type Output = ClaimOutcome;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<ClaimOutcome, sqlx::Error> {
        let owner = owner_of(&self.cookie_value);
        let expires_at = self.at.plus_millis(CLAIM_TTL_MILLIS);

        if renew(conn, &owner, self.at, expires_at).await? {
            return Ok(ClaimOutcome::Granted { expires_at });
        }
        if take(conn, &owner, self.at, expires_at).await? {
            return Ok(ClaimOutcome::Granted { expires_at });
        }

        // Not ours and not expired: report when it lapses, and change nothing.
        let held_until = current_expiry(conn).await?.unwrap_or(self.at);
        Ok(ClaimOutcome::Blocked {
            expires_at: held_until,
        })
    }
}

/// Slides a claim this cookie already holds ten minutes further out.
///
/// Renewal is not a separate mechanism: every claim-gated request that succeeds
/// runs this, so an operator who keeps working never meets the timeout and one
/// who walks away releases the wizard without doing anything.
#[derive(Debug)]
pub struct RenewClaim {
    /// The cookie value the caller presented.
    pub cookie_value: String,
    /// The instant of the request.
    pub at: Timestamp,
}

impl WriteOperation for RenewClaim {
    type Output = Option<Timestamp>;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<Option<Timestamp>, sqlx::Error> {
        let owner = owner_of(&self.cookie_value);
        let expires_at = self.at.plus_millis(CLAIM_TTL_MILLIS);
        Ok(renew(conn, &owner, self.at, expires_at)
            .await?
            .then_some(expires_at))
    }
}

async fn renew(
    conn: &mut SqliteConnection,
    owner: &LeaseOwner,
    at: Timestamp,
    expires_at: Timestamp,
) -> Result<bool, sqlx::Error> {
    Heartbeat {
        name: LeaseName::SetupClaim,
        owner: owner.clone(),
        at,
        expires_at,
    }
    .execute(conn)
    .await
}

async fn take(
    conn: &mut SqliteConnection,
    owner: &LeaseOwner,
    at: Timestamp,
    expires_at: Timestamp,
) -> Result<bool, sqlx::Error> {
    Acquire {
        name: LeaseName::SetupClaim,
        owner: owner.clone(),
        at,
        expires_at,
    }
    .execute(conn)
    .await
}

async fn current_expiry(conn: &mut SqliteConnection) -> Result<Option<Timestamp>, sqlx::Error> {
    let name = LeaseName::SetupClaim.as_text();
    Ok(
        sqlx::query_scalar!("SELECT expires_at FROM leases WHERE name = ?1", name)
            .fetch_optional(&mut *conn)
            .await?
            .map(Timestamp::from_millis),
    )
}
