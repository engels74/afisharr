// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `users` table.

use sqlx::{SqliteConnection, SqlitePool};

use crate::{
    accounts::{AccountError, User, UserKind},
    identifier::Id,
    storage::WriteOperation,
    time::Timestamp,
};

/// Whether any account holds administrator rights.
///
/// The first-run rule turns on this one question (PRD §21.4.2): nothing is
/// reachable until an admin exists, and no admin is creatable once one does.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn admin_exists(readers: &SqlitePool) -> Result<bool, sqlx::Error> {
    let found = sqlx::query_scalar!(
        "SELECT 1 FROM users WHERE is_admin = 1 AND disabled_at IS NULL LIMIT 1"
    )
    .fetch_optional(readers)
    .await?;
    Ok(found.is_some())
}

/// Reads the account with this username, if one exists.
///
/// # Errors
/// Returns [`AccountError::UnknownKind`] when the row holds a `kind` this
/// binary does not know, and [`AccountError::Storage`] when the read fails.
pub async fn find_by_username(
    readers: &SqlitePool,
    username: &str,
) -> Result<Option<User>, AccountError> {
    sqlx::query_as!(Row, "SELECT * FROM users WHERE username = ?1", username)
        .fetch_optional(readers)
        .await
        .map_err(|source| AccountError::Storage(source.into()))?
        .map(User::try_from)
        .transpose()
}

/// Reads the account with this identifier, if one exists.
///
/// # Errors
/// Returns [`AccountError::UnknownKind`] when the row holds a `kind` this
/// binary does not know, and [`AccountError::Storage`] when the read fails.
pub async fn find_by_id(readers: &SqlitePool, id: &str) -> Result<Option<User>, AccountError> {
    sqlx::query_as!(Row, "SELECT * FROM users WHERE id = ?1", id)
        .fetch_optional(readers)
        .await
        .map_err(|source| AccountError::Storage(source.into()))?
        .map(User::try_from)
        .transpose()
}

/// Reads the account bound to this plex.tv account id, if one exists.
///
/// # Errors
/// Returns [`AccountError::UnknownKind`] when the row holds a `kind` this
/// binary does not know, and [`AccountError::Storage`] when the read fails.
pub async fn find_by_plex_account(
    readers: &SqlitePool,
    plex_account_id: i64,
) -> Result<Option<User>, AccountError> {
    sqlx::query_as!(
        Row,
        "SELECT * FROM users WHERE plex_account_id = ?1",
        plex_account_id
    )
    .fetch_optional(readers)
    .await
    .map_err(|source| AccountError::Storage(source.into()))?
    .map(User::try_from)
    .transpose()
}

/// What creating a local account produced.
///
/// Two of the three outcomes are refusals the caller must render differently,
/// so they are values rather than errors: "an administrator already exists" is
/// the first-run rule working, and "that username is taken" is a form the
/// operator can correct. Neither is a fault.
#[derive(Debug)]
pub enum CreateUserOutcome {
    /// The account now exists.
    Created(Box<User>),
    /// An enabled administrator already exists, so no second one was created.
    AdminAlreadyExists,
    /// Another account already holds that username.
    UsernameTaken,
}

/// Creates a local account, refusing a second administrator.
///
/// The refusal is inside the write operation rather than in front of it,
/// because "no admin exists" read on a read connection and "no admin exists"
/// at the moment of the insert are different facts. The write actor
/// serialises every mutation (D-024), so checking here makes the pair atomic.
#[derive(Debug)]
pub struct CreateUser {
    /// The identifier to assign.
    pub id: Id,
    /// The name the account signs in with.
    pub username: String,
    /// The Argon2id PHC string.
    pub password_hash: String,
    /// Whether this account holds administrator rights.
    pub is_admin: bool,
    /// The instant of the write.
    pub at: Timestamp,
}

impl WriteOperation for CreateUser {
    type Output = Result<CreateUserOutcome, AccountError>;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<Self::Output, sqlx::Error> {
        let id = self.id.as_str().to_owned();
        let at = self.at.as_millis();
        let is_admin = i64::from(self.is_admin);

        if self.is_admin {
            let existing = sqlx::query_scalar!(
                "SELECT 1 FROM users WHERE is_admin = 1 AND disabled_at IS NULL LIMIT 1"
            )
            .fetch_optional(&mut *conn)
            .await?;
            if existing.is_some() {
                return Ok(Ok(CreateUserOutcome::AdminAlreadyExists));
            }
        }

        let inserted = sqlx::query!(
            "INSERT INTO users (id, kind, username, password_hash, is_admin, created_at, updated_at)
             VALUES (?1, 'Local', ?2, ?3, ?4, ?5, ?5)",
            id,
            self.username,
            self.password_hash,
            is_admin,
            at
        )
        .execute(&mut *conn)
        .await;

        // `ux_users__username` is the only unique index this statement can
        // violate, so a constraint failure here is that name being taken.
        if let Err(error) = inserted {
            if is_unique_violation(&error) {
                return Ok(Ok(CreateUserOutcome::UsernameTaken));
            }
            return Err(error);
        }

        let row = sqlx::query_as!(Row, "SELECT * FROM users WHERE id = ?1", id)
            .fetch_one(&mut *conn)
            .await?;
        Ok(User::try_from(row).map(|user| CreateUserOutcome::Created(Box::new(user))))
    }
}

/// Whether `SQLite` refused a statement for violating a unique index.
fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(
        error,
        sqlx::Error::Database(inner) if inner.is_unique_violation()
    )
}

/// Creates or refreshes the account behind a plex.tv login.
///
/// `plex_account_id` is the binding, never the key (P4): the row keeps its ULID
/// across a rename on plex.tv, and a returning account matches on the numeric
/// id rather than on whatever display name it now carries.
#[derive(Debug)]
pub struct UpsertPlexUser {
    /// The identifier to assign if this account is new.
    pub id: Id,
    /// plex.tv's numeric account id.
    pub plex_account_id: i64,
    /// plex.tv's account uuid.
    pub plex_uuid: Option<String>,
    /// The name to show and sign in as.
    pub username: String,
    /// Contact address, when plex.tv reported one.
    pub email: Option<String>,
    /// Avatar, when plex.tv reported one.
    pub avatar_url: Option<String>,
    /// Whether this account holds administrator rights.
    pub is_admin: bool,
    /// The instant of the write.
    pub at: Timestamp,
}

impl WriteOperation for UpsertPlexUser {
    type Output = Result<User, AccountError>;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<Self::Output, sqlx::Error> {
        let id = self.id.as_str().to_owned();
        let at = self.at.as_millis();
        let is_admin = i64::from(self.is_admin);

        sqlx::query!(
            "INSERT INTO users (
                 id, kind, username, email, plex_account_id, plex_uuid, avatar_url,
                 is_admin, created_at, updated_at)
             VALUES (?1, 'Plex', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
             ON CONFLICT(plex_account_id) WHERE plex_account_id IS NOT NULL DO UPDATE SET
                 username   = excluded.username,
                 email      = excluded.email,
                 plex_uuid  = excluded.plex_uuid,
                 avatar_url = excluded.avatar_url,
                 updated_at = excluded.updated_at",
            id,
            self.username,
            self.email,
            self.plex_account_id,
            self.plex_uuid,
            self.avatar_url,
            is_admin,
            at
        )
        .execute(&mut *conn)
        .await?;

        let row = sqlx::query_as!(
            Row,
            "SELECT * FROM users WHERE plex_account_id = ?1",
            self.plex_account_id
        )
        .fetch_one(&mut *conn)
        .await?;
        Ok(User::try_from(row))
    }
}

/// Replaces one account's password hash.
#[derive(Debug)]
pub struct SetPassword {
    /// The account being changed.
    pub user_id: String,
    /// The new Argon2id PHC string.
    pub password_hash: String,
    /// The instant of the write.
    pub at: Timestamp,
}

impl WriteOperation for SetPassword {
    type Output = bool;

    async fn execute(self, conn: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
        let at = self.at.as_millis();
        let affected = sqlx::query!(
            "UPDATE users SET password_hash = ?2, updated_at = ?3
             WHERE id = ?1 AND kind = 'Local'",
            self.user_id,
            self.password_hash,
            at
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();
        Ok(affected == 1)
    }
}

/// Records that an account signed in.
#[derive(Debug)]
pub struct TouchLastLogin {
    /// The account that signed in.
    pub user_id: String,
    /// The instant of the sign-in.
    pub at: Timestamp,
}

impl WriteOperation for TouchLastLogin {
    type Output = ();

    async fn execute(self, conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        let at = self.at.as_millis();
        sqlx::query!(
            "UPDATE users SET last_login_at = ?2 WHERE id = ?1",
            self.user_id,
            at
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

/// The `users` row exactly as `SQLite` returns it.
struct Row {
    id: String,
    kind: String,
    username: String,
    email: Option<String>,
    display_name: Option<String>,
    password_hash: Option<String>,
    plex_account_id: Option<i64>,
    plex_uuid: Option<String>,
    avatar_url: Option<String>,
    is_admin: i64,
    created_at: i64,
    updated_at: i64,
    last_login_at: Option<i64>,
    disabled_at: Option<i64>,
}

impl TryFrom<Row> for User {
    type Error = AccountError;

    fn try_from(row: Row) -> Result<Self, Self::Error> {
        let kind = UserKind::from_text(&row.kind).ok_or_else(|| AccountError::UnknownKind {
            id: row.id.clone(),
            kind: row.kind.clone(),
        })?;
        Ok(Self {
            id: row.id,
            kind,
            username: row.username,
            email: row.email,
            display_name: row.display_name,
            password_hash: row.password_hash,
            plex_account_id: row.plex_account_id,
            plex_uuid: row.plex_uuid,
            avatar_url: row.avatar_url,
            is_admin: row.is_admin != 0,
            created_at: Timestamp::from_millis(row.created_at),
            updated_at: Timestamp::from_millis(row.updated_at),
            last_login_at: row.last_login_at.map(Timestamp::from_millis),
            disabled_at: row.disabled_at.map(Timestamp::from_millis),
        })
    }
}
