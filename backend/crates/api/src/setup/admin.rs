// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `POST /api/setup/admin` — the first-run administrator account.

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use afisharr_core::{
    accounts::{self, CreateUser, CreateUserOutcome},
    identifier::Id,
};
use axum::{Json, extract::State};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    authentication::SignedIn,
    error::{AppError, AppResult, ErrorCode, JsonBody, Problem},
    setup::events::record_step,
    state::ApiState,
};

/// The shortest password this instance will store.
///
/// A floor rather than a composition rule: length is the property that
/// survives contact with how people actually choose passwords, and a rule
/// demanding a digit and a symbol produces `Password1!` on every instance in
/// the world. Declared once, beside the change-password route that enforces
/// the same rule (P7).
use crate::authentication::account_routes::MINIMUM_PASSWORD_LENGTH;

/// The administrator the wizard creates.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAdmin {
    /// The name the account will sign in with.
    pub username: String,
    /// The password.
    pub password: String,
}

/// Creates the administrator account.
///
/// Reachable only behind the claim gate, and only while no administrator
/// exists. The moment this page is reachable on an instance that may face the
/// internet it is an unauthenticated grant of administrator, which is why the
/// gate arrives with it rather than eleven phases later (D-045).
#[utoipa::path(
    post,
    path = "/api/setup/admin",
    tag = "setup",
    request_body = CreateAdmin,
    responses(
        (status = 200, description = "The administrator account now exists", body = SignedIn),
        (status = 400, description = "The username or password was refused", body = Problem),
        (status = 409, description = "An administrator already exists", body = Problem),
    ),
)]
pub async fn create_admin(
    State(state): State<ApiState>,
    JsonBody(request): JsonBody<CreateAdmin>,
) -> AppResult<Json<SignedIn>> {
    let username = request.username.trim().to_owned();
    validate(&username, &request.password)?;

    let password_hash = accounts::hash(request.password)
        .await
        .map_err(AppError::internal)?;

    let outcome = state
        .database()
        .writer()
        .submit(CreateUser {
            id: Id::generate(state.clock()),
            username: username.clone(),
            password_hash,
            is_admin: true,
            at: state.clock().now(),
        })
        .await
        .map_err(AppError::internal)?
        .map_err(AppError::internal)?;

    match outcome {
        CreateUserOutcome::Created(user) => {
            record_step(&state, "admin", "The administrator account was created.").await;
            Ok(Json(SignedIn::from(user.as_ref())))
        }
        CreateUserOutcome::AdminAlreadyExists => Err(AppError::of(
            ErrorCode::Conflict,
            "This instance already has an administrator. Sign in instead.",
        )),
        CreateUserOutcome::UsernameTaken => Err(AppError::new(
            Problem::new(ErrorCode::Conflict, "That username is already taken.").at("/username"),
        )),
    }
}

/// Refuses a username or password the instance will not store.
///
/// Both refusals carry a JSON pointer, so the form puts the message beside the
/// field rather than in a banner naming nothing (PRD §8.4).
fn validate(username: &str, password: &str) -> AppResult<()> {
    if username.is_empty() {
        return Err(AppError::new(
            Problem::new(ErrorCode::Invalid, "A username is required.").at("/username"),
        ));
    }
    if username.chars().count() > 64 {
        return Err(AppError::new(
            Problem::new(ErrorCode::Invalid, "That username is too long.")
                .at("/username")
                .expecting(
                    "at most 64 characters",
                    format!("{} characters", username.chars().count()),
                ),
        ));
    }
    if password.chars().count() < MINIMUM_PASSWORD_LENGTH {
        return Err(AppError::new(
            Problem::new(ErrorCode::Invalid, "That password is too short.")
                .at("/password")
                .expecting(
                    format!("at least {MINIMUM_PASSWORD_LENGTH} characters"),
                    format!("{} characters", password.chars().count()),
                ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reasonable_account_validates() {
        assert!(validate("operator", "correct horse battery staple").is_ok());
    }

    #[test]
    fn an_empty_username_is_refused_at_its_own_field() {
        let error = validate("", "correct horse battery staple")
            .expect_err("an empty username must be refused");
        assert_eq!(error.problem().pointer.as_deref(), Some("/username"));
    }

    #[test]
    fn a_short_password_is_refused_with_what_was_expected() {
        let error = validate("operator", "short").expect_err("a short password must be refused");
        assert_eq!(error.problem().pointer.as_deref(), Some("/password"));
        let mismatch = error
            .problem()
            .mismatch
            .as_ref()
            .expect("the refusal must say what was expected");
        assert_eq!(mismatch.expected, "at least 12 characters");
        assert_eq!(mismatch.actual, "5 characters");
    }

    #[test]
    fn the_password_floor_is_counted_in_characters_rather_than_bytes() {
        // Twelve emoji are twelve characters and forty-eight bytes; a byte
        // count would accept four of them.
        let twelve = "🔒".repeat(12);
        assert!(validate("operator", &twelve).is_ok());
        let four = "🔒".repeat(4);
        assert!(validate("operator", &four).is_err());
    }

    #[test]
    fn the_request_body_rejects_a_field_it_does_not_know() {
        assert!(
            serde_json::from_str::<CreateAdmin>(
                r#"{"username":"a","password":"b","isAdmin":false}"#
            )
            .is_err()
        );
    }
}
