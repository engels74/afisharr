// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading and refreshing the `plex_server` row.

use sqlx::{SqliteConnection, SqlitePool};

use crate::{storage::WriteOperation, time::Timestamp};

/// The server this installation is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlexServer {
    /// The identity every Plex-bound row is scoped to.
    pub machine_identifier: String,
    /// The name the operator gave the server.
    pub friendly_name: String,
    /// The server version, which invalidates the discovered field cache when it
    /// changes (PRD §19.8).
    pub version: String,
    /// The platform it runs on, when it reports one.
    pub platform: Option<String>,
    /// Where this installation reaches it.
    pub base_url: String,
    /// The plex.tv account that owns it, when known.
    pub owner_account_id: Option<i64>,
    /// When it was first bound.
    pub first_seen_at: Timestamp,
    /// When it last answered.
    pub last_seen_at: Timestamp,
    /// When its version last changed.
    pub last_version_change_at: Option<Timestamp>,
}

/// Reads the bound server, if this installation has one.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn load(readers: &SqlitePool) -> Result<Option<PlexServer>, sqlx::Error> {
    Ok(sqlx::query_as!(
        Row,
        "SELECT machine_identifier, friendly_name, version, platform, base_url,
                owner_account_id, first_seen_at, last_seen_at, last_version_change_at
         FROM plex_server WHERE id = 1"
    )
    .fetch_optional(readers)
    .await?
    .map(PlexServer::from))
}

/// What one successful observation of the bound server changes.
///
/// Never the machine identifier. On a first bind the row is inserted with the
/// identifier that answered; afterwards the update is scoped to that same
/// identifier, so an observation of a *different* server matches no row and
/// writes nothing — the zero-writes half of `I-ID-5`, enforced by the statement
/// rather than by every caller remembering to check first.
///
/// Never backwards, either. [`RecordObservation::at`] is stamped before the
/// request goes out, so two overlapping checks can finish in the order they did
/// not start in; the update is scoped to the recorded instant as well, and an
/// observation older than the one already stored writes nothing rather than
/// replacing a newer server description with a staler one.
#[derive(Debug, Clone)]
pub struct RecordObservation {
    /// The identifier that answered.
    pub machine_identifier: String,
    /// The name it reported.
    pub friendly_name: String,
    /// The version it reported.
    pub version: String,
    /// The platform it reported, if any.
    pub platform: Option<String>,
    /// The address it answered at.
    pub base_url: String,
    /// The instant of the observation.
    pub at: Timestamp,
}

/// What one observation did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed {
    /// Nothing was bound; this server is now.
    Bound,
    /// The bound server answered, and its record was refreshed.
    Refreshed,
    /// A different server answered. Nothing was written.
    Ignored,
    /// The bound server answered, and a later observation is already recorded.
    ///
    /// Its own outcome and not [`Self::Refreshed`], because nothing was
    /// refreshed: a caller told the record was updated would report a stored
    /// version that is not the one it just wrote.
    Stale,
}

impl WriteOperation for RecordObservation {
    type Output = Observed;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<Observed, sqlx::Error> {
        let at = self.at.as_millis();
        let Self {
            machine_identifier,
            friendly_name,
            version,
            platform,
            base_url,
            ..
        } = self;

        // Scoped to the identifier that answered, so an observation of another
        // server updates no row. `last_version_change_at` moves only when the
        // version really changed: it is what invalidates the discovered field
        // cache, and stamping it every pass would rediscover the whole field
        // vocabulary on every check (PRD §19.8).
        //
        // Scoped to `last_seen_at` too, and that is not the same condition. The
        // instant is taken before the request, so the check that started first
        // can finish last: without this clause its answer would overwrite the
        // newer one's version, name, platform, and `last_seen_at` — a record
        // that goes backwards while both checks report success. `<=` rather
        // than `<`, so a re-check inside the same millisecond still refreshes.
        let updated = sqlx::query!(
            "UPDATE plex_server
                SET friendly_name = ?2,
                    platform      = ?4,
                    base_url      = ?5,
                    last_seen_at  = ?6,
                    last_version_change_at =
                        CASE WHEN version = ?3 THEN last_version_change_at ELSE ?6 END,
                    version       = ?3
              WHERE id = 1 AND machine_identifier = ?1 AND last_seen_at <= ?6",
            machine_identifier,
            friendly_name,
            version,
            platform,
            base_url,
            at
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();
        if updated > 0 {
            return Ok(Observed::Refreshed);
        }

        // No row matched, which now has one more meaning than it used to: the
        // bound server may be recorded already, from an observation newer than
        // this one. Reported as itself rather than folded into either outcome
        // below — it is neither a first bind nor a different server, and a
        // caller that read it as `Ignored` would be told `I-ID-5` had fired.
        let recorded = sqlx::query_scalar!(
            "SELECT last_seen_at FROM plex_server WHERE id = 1 AND machine_identifier = ?1",
            machine_identifier
        )
        .fetch_optional(&mut *conn)
        .await?;
        if recorded.is_some() {
            return Ok(Observed::Stale);
        }

        // Either nothing is bound — in which case this is the first bind — or
        // something else is, and this observation is of a server this
        // installation is not bound to. `INSERT` on the constrained primary key
        // distinguishes the two without a second read.
        let inserted = sqlx::query!(
            "INSERT INTO plex_server (
                 id, machine_identifier, friendly_name, version, platform, base_url,
                 first_seen_at, last_seen_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(id) DO NOTHING",
            machine_identifier,
            friendly_name,
            version,
            platform,
            base_url,
            at
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();

        Ok(if inserted > 0 {
            Observed::Bound
        } else {
            Observed::Ignored
        })
    }
}

/// The `plex_server` row exactly as `SQLite` returns it.
struct Row {
    machine_identifier: String,
    friendly_name: String,
    version: String,
    platform: Option<String>,
    base_url: String,
    owner_account_id: Option<i64>,
    first_seen_at: i64,
    last_seen_at: i64,
    last_version_change_at: Option<i64>,
}

impl From<Row> for PlexServer {
    fn from(row: Row) -> Self {
        Self {
            machine_identifier: row.machine_identifier,
            friendly_name: row.friendly_name,
            version: row.version,
            platform: row.platform,
            base_url: row.base_url,
            owner_account_id: row.owner_account_id,
            first_seen_at: Timestamp::from_millis(row.first_seen_at),
            last_seen_at: Timestamp::from_millis(row.last_seen_at),
            last_version_change_at: row.last_version_change_at.map(Timestamp::from_millis),
        }
    }
}
