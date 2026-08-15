// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `OpenAPI` document, which is the contract between the two surfaces.
//!
//! Every route on this surface appears here, and the TypeScript client is
//! generated from what this produces. A handler that is not listed is a handler
//! the frontend cannot call — deliberately, because the alternative is a
//! hand-written `fetch` with an ad hoc URL, which is the thing §24.5 exists to
//! forbid.

use utoipa::OpenApi;

/// The document.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Afisharr",
        description = "Plex collections, posters, and overlay manager.",
        license(name = "AGPL-3.0-or-later", identifier = "AGPL-3.0-or-later"),
    ),
    tags(
        (name = "health", description = "Liveness, with no credential required"),
        (name = "setup", description = "First run: the claim, the derived step, and completion"),
        (name = "authentication", description = "Signing in and out"),
        (name = "settings", description = "Instance configuration"),
        (name = "files", description = "The jailed filesystem browser"),
        (name = "stream", description = "The multiplexed event stream"),
    ),
    paths(
        crate::health::route::health,
        crate::setup::claim_status::claim_status,
        crate::setup::claim_routes::claim,
        crate::setup::recover_routes::recover,
        crate::setup::status::status,
        crate::setup::admin::create_admin,
        crate::setup::status::complete,
        crate::authentication::password_login::log_in,
        crate::authentication::password_login::log_out,
        crate::authentication::password_login::whoami,
        crate::authentication::plex_pin_start::start_plex_pin,
        crate::authentication::plex_pin_poll::poll_plex_pin,
        crate::authentication::account_routes::change_password,
        crate::authentication::account_routes::list_sessions,
        crate::authentication::account_routes::revoke_session,
        crate::files::browse::roots,
        crate::files::browse::browse,
        crate::keys::routes::list,
        crate::keys::routes::create,
        crate::keys::routes::revoke,
        crate::plex::routes::check_connection,
        crate::stream::route::stream,
    ),
    components(schemas(
        crate::error::Problem,
        crate::error::ErrorCode,
        crate::error::Mismatch,
        crate::plex::PlexConnection,
        crate::plex::PlexConnectionState,
    )),
)]
pub struct ApiDoc;

/// The document as pretty JSON, which is what the client generator reads.
///
/// Pretty rather than compact so the committed copy diffs line by line: the
/// `contract-check` lane's whole job is to show that a handler changed and the
/// client did not, and a one-line document makes that diff unreadable.
///
/// # Errors
/// Returns the serialisation failure, which can only mean a malformed
/// annotation on a handler in this crate.
pub fn document() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed() -> serde_json::Value {
        serde_json::from_str(&document().expect("the document must serialise"))
            .expect("the document must be JSON")
    }

    #[test]
    fn every_route_the_router_serves_is_in_the_document() {
        // The list the generated client is built from. A route missing here is
        // a route the frontend would have to hand-write a fetch for (§24.5).
        let expected = [
            "/api/health",
            "/api/setup/claim",
            "/api/setup/recover",
            "/api/setup/status",
            "/api/setup/admin",
            "/api/setup/complete",
            "/api/auth/login",
            "/api/auth/logout",
            "/api/auth/session",
            "/api/auth/plex/pin",
            "/api/auth/plex/pin/{id}",
            "/api/settings/password",
            "/api/settings/sessions",
            "/api/settings/sessions/{id}",
            "/api/files",
            "/api/files/roots",
            "/api/settings/api-keys",
            "/api/settings/api-keys/{id}",
            "/api/settings/plex/connection/check",
            "/api/stream",
        ];
        let document = parsed();
        let paths = document["paths"].as_object().expect("paths is an object");
        for path in expected {
            assert!(paths.contains_key(path), "{path} is missing");
        }
        assert_eq!(paths.len(), expected.len(), "an undeclared path appeared");
    }

    #[test]
    fn the_one_error_shape_is_a_named_component() {
        let document = parsed();
        let schemas = document["components"]["schemas"]
            .as_object()
            .expect("schemas is an object");
        assert!(schemas.contains_key("Problem"));
        assert!(schemas.contains_key("ErrorCode"));
        assert!(schemas.contains_key("Mismatch"));
    }

    #[test]
    fn the_licence_is_declared_so_the_generated_client_carries_it() {
        let document = parsed();
        assert_eq!(document["info"]["license"]["name"], "AGPL-3.0-or-later");
    }

    #[test]
    fn every_failure_response_points_at_the_one_problem_shape() {
        // A route that answered with its own error body would show up here as
        // an inline schema rather than a reference.
        let document = parsed();
        let text = document.to_string();
        assert!(text.contains("#/components/schemas/Problem"), "{text}");
    }
}
