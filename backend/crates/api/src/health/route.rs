// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `GET /api/health`.

use axum::{Json, extract::State};
use serde::Serialize;
use utoipa::ToSchema;

use crate::state::ApiState;

/// What the health route reports.
///
/// Deliberately thin. A container orchestrator asks this before anyone has
/// signed in, so it must not name libraries, integrations, or anything else
/// that describes the operator's setup — this is the one route with no
/// credential in front of it (`I-SEC-8` excepts it by name).
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    /// Always `ok` when this route answers at all.
    pub status: &'static str,
    /// The running binary's version.
    pub version: String,
    /// Whether first-run setup has finished.
    ///
    /// The one fact beyond liveness, and it is already observable from outside:
    /// an unclaimed instance answers every other route with `setupRequired`.
    /// Reporting it here lets the interface route to the claim page on its
    /// first load instead of learning it from a refusal.
    pub setup_completed: bool,
}

/// Reports that the instance is up.
///
/// The 429 is declared because the group carries a rate limit, and a layer
/// answering on a route's behalf must be in that route's own annotations: the
/// generated client is built from them and nothing else, so an undeclared answer
/// is one the interface simply does not handle (§24.5).
#[utoipa::path(
    get,
    path = "/api/health",
    tag = "health",
    responses(
        (status = 200, description = "The instance is serving", body = Health),
        (
            status = 429,
            description = "Too many requests",
            body = crate::error::Problem,
        ),
    ),
)]
pub async fn health(State(state): State<ApiState>) -> Json<Health> {
    Json(Health {
        status: "ok",
        version: state.identity().app_version.clone(),
        setup_completed: state.setup_completed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_names_nothing_about_the_operators_setup() {
        let encoded = serde_json::to_value(Health {
            status: "ok",
            version: "0.1.0".to_owned(),
            setup_completed: false,
        })
        .expect("serialises");
        let mut keys: Vec<&str> = encoded
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["setupCompleted", "status", "version"]);
    }
}
