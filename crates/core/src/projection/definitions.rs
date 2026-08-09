// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Projecting a definition envelope onto the `definitions` derived columns.

use serde_json::Value;

use crate::{digest, projection::ProjectionError};

/// Every derived column on `definitions`, computed from `body_json`.
///
/// The definition body is the source of truth; each of these exists only so an
/// index can reach it (PRD §19.9). Nothing but [`project_definition`] writes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionColumns {
    /// The document kind.
    pub kind: String,
    /// `namespace/slug`.
    pub handle: String,
    /// The display name.
    pub name: String,
    /// Which body format this document is written in.
    pub schema_version: i64,
    /// Which registry revision validated it.
    pub registry_version: i64,
    /// The digest of the canonical body — the optimistic-concurrency token.
    pub body_hash: String,
    /// `User` or `Pack`.
    pub origin_kind: String,
    /// The pack namespace, when the origin is a pack.
    pub origin_pack: Option<String>,
    /// The pack version, when the origin is a pack.
    pub origin_pack_version: Option<String>,
}

/// Projects one definition body onto its derived columns.
///
/// # Errors
/// Returns [`ProjectionError`] naming the row and the JSON pointer when the
/// envelope is not JSON, is missing a field, or holds an origin type outside
/// the set the column allows.
pub fn project_definition(id: &str, body_json: &str) -> Result<DefinitionColumns, ProjectionError> {
    let body: Value =
        serde_json::from_str(body_json).map_err(|source| ProjectionError::NotJson {
            id: id.to_owned(),
            source,
        })?;

    let origin = body.pointer("/meta/origin");
    let origin_type = required_str(
        id,
        origin.unwrap_or(&Value::Null),
        "type",
        "/meta/origin/type",
    )?;

    let (origin_kind, origin_pack, origin_pack_version) = match origin_type {
        "user" => ("User".to_owned(), None, None),
        "pack" => (
            "Pack".to_owned(),
            // A pack-originated document that does not name its pack cannot be
            // matched against an installed pack, so it is a corrupt envelope
            // rather than a document with an unknown pack.
            Some(
                required_str(
                    id,
                    origin.unwrap_or(&Value::Null),
                    "pack",
                    "/meta/origin/pack",
                )?
                .to_owned(),
            ),
            origin
                .and_then(|origin| origin.get("packVersion"))
                .and_then(Value::as_str)
                .map(str::to_owned),
        ),
        found => {
            return Err(ProjectionError::UnexpectedValue {
                id: id.to_owned(),
                pointer: "/meta/origin/type".to_owned(),
                found: found.to_owned(),
                expected: "'user' or 'pack'".to_owned(),
            });
        }
    };

    let canonical = digest::canonicalize(body_json).map_err(|source| ProjectionError::NotJson {
        id: id.to_owned(),
        source,
    })?;

    Ok(DefinitionColumns {
        kind: required_str(id, &body, "kind", "/kind")?.to_owned(),
        handle: required_str(id, &body, "handle", "/handle")?.to_owned(),
        name: required_str(id, &body, "name", "/name")?.to_owned(),
        schema_version: required_i64(id, &body, "schemaVersion", "/schemaVersion")?,
        registry_version: required_i64(id, &body, "registryVersion", "/registryVersion")?,
        body_hash: digest::hex(canonical),
        origin_kind,
        origin_pack,
        origin_pack_version,
    })
}

fn required_str<'a>(
    id: &str,
    value: &'a Value,
    key: &str,
    pointer: &str,
) -> Result<&'a str, ProjectionError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ProjectionError::MissingField {
            id: id.to_owned(),
            pointer: pointer.to_owned(),
        })
}

fn required_i64(id: &str, value: &Value, key: &str, pointer: &str) -> Result<i64, ProjectionError> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| ProjectionError::MissingField {
            id: id.to_owned(),
            pointer: pointer.to_owned(),
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const USER_BODY: &str = r#"{
        "kind": "Collection",
        "schemaVersion": 1,
        "registryVersion": 3,
        "id": "01J9Z7Q0K8Y3X2W1V0U9T8S7R6",
        "handle": "user/trending-now",
        "name": "Trending Now",
        "meta": { "origin": { "type": "user" }, "tags": [] },
        "spec": {}
    }"#;

    const PACK_BODY: &str = r#"{
        "kind": "OverlayTemplate",
        "schemaVersion": 2,
        "registryVersion": 3,
        "id": "01J9Z7Q0K8Y3X2W1V0U9T8S7R7",
        "handle": "afisharr.media-info/4k-badge",
        "name": "4K Badge",
        "meta": { "origin": { "type": "pack", "pack": "afisharr.media-info", "packVersion": "1.2.0" } },
        "spec": {}
    }"#;

    #[test]
    fn a_user_definition_projects_every_column() {
        let columns = project_definition("D1", USER_BODY).unwrap();
        assert_eq!(columns.kind, "Collection");
        assert_eq!(columns.handle, "user/trending-now");
        assert_eq!(columns.name, "Trending Now");
        assert_eq!(columns.schema_version, 1);
        assert_eq!(columns.registry_version, 3);
        assert_eq!(columns.origin_kind, "User");
        assert_eq!(columns.origin_pack, None);
        assert_eq!(columns.origin_pack_version, None);
        assert_eq!(columns.body_hash.len(), 64);
    }

    #[test]
    fn a_pack_definition_carries_its_pack_and_version() {
        let columns = project_definition("D2", PACK_BODY).unwrap();
        assert_eq!(columns.origin_kind, "Pack");
        assert_eq!(columns.origin_pack.as_deref(), Some("afisharr.media-info"));
        assert_eq!(columns.origin_pack_version.as_deref(), Some("1.2.0"));
    }

    #[test]
    fn the_body_hash_ignores_formatting_and_key_order() {
        let reformatted = r#"{"spec":{},"meta":{"tags":[],"origin":{"type":"user"}},
            "name":"Trending Now","handle":"user/trending-now",
            "id":"01J9Z7Q0K8Y3X2W1V0U9T8S7R6","registryVersion":3,"schemaVersion":1,
            "kind":"Collection"}"#;
        assert_eq!(
            project_definition("D1", USER_BODY).unwrap().body_hash,
            project_definition("D1", reformatted).unwrap().body_hash
        );
    }

    #[test]
    fn the_body_hash_changes_when_the_document_does() {
        let edited = USER_BODY.replace("Trending Now", "Trending Later");
        assert_ne!(
            project_definition("D1", USER_BODY).unwrap().body_hash,
            project_definition("D1", &edited).unwrap().body_hash
        );
    }

    #[test]
    fn a_missing_field_is_named_by_json_pointer() {
        let without_handle = USER_BODY.replace("\"handle\"", "\"handel\"");
        assert!(matches!(
            project_definition("D1", &without_handle),
            Err(ProjectionError::MissingField { pointer, .. }) if pointer == "/handle"
        ));
    }

    #[test]
    fn an_unknown_origin_type_is_refused_rather_than_defaulted() {
        let odd_origin = USER_BODY.replace("\"user\"", "\"builtin\"");
        assert!(matches!(
            project_definition("D1", &odd_origin),
            Err(ProjectionError::UnexpectedValue { found, .. }) if found == "builtin"
        ));
    }

    #[test]
    fn a_pack_origin_with_no_pack_named_is_refused() {
        let anonymous = PACK_BODY.replace("\"pack\": \"afisharr.media-info\",", "");
        assert!(matches!(
            project_definition("D2", &anonymous),
            Err(ProjectionError::MissingField { pointer, .. }) if pointer == "/meta/origin/pack"
        ));
    }

    #[test]
    fn a_body_that_is_not_json_is_refused() {
        assert!(matches!(
            project_definition("D1", "not json"),
            Err(ProjectionError::NotJson { .. })
        ));
    }
}
