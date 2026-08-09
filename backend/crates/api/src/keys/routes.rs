// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `/api/settings/api-keys`.

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use afisharr_core::{
    api_keys::{self, ApiKeyRecord, CreateApiKey, IssuedApiKey, RevokeApiKey},
    identifier::Id,
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    authentication::Administrator,
    error::{AppError, AppResult, ErrorCode, JsonBody, Problem},
    state::ApiState,
};

/// What the operator names a new key.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewApiKey {
    /// A name the operator will recognise later.
    pub name: String,
}

/// A key as the list shows it.
///
/// No field here authenticates anything. The prefix tells two keys apart and
/// the digest is not in the shape at all, so an operator who screenshots this
/// page has leaked nothing.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyView {
    /// The key's identifier, which is what revocation names.
    pub id: String,
    /// The operator's name for it.
    pub name: String,
    /// The first eight characters, for display.
    pub prefix: String,
    /// When it was issued, in epoch milliseconds.
    pub created_at: i64,
    /// When it was last accepted, in epoch milliseconds.
    pub last_used_at: Option<i64>,
    /// When it was revoked, in epoch milliseconds.
    pub revoked_at: Option<i64>,
}

impl From<ApiKeyRecord> for ApiKeyView {
    fn from(record: ApiKeyRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            prefix: record.prefix,
            created_at: record.created_at.as_millis(),
            last_used_at: record
                .last_used_at
                .map(afisharr_core::time::Timestamp::as_millis),
            revoked_at: record
                .revoked_at
                .map(afisharr_core::time::Timestamp::as_millis),
        }
    }
}

/// A key at the one moment its plaintext exists.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssuedKey {
    /// The key as the list will show it from now on.
    #[serde(flatten)]
    pub key: ApiKeyView,
    /// The plaintext. Shown once; this instance cannot produce it again.
    pub secret: String,
}

/// Lists every key.
#[utoipa::path(
    get,
    path = "/api/settings/api-keys",
    tag = "settings",
    responses(
        (status = 200, description = "Every key, newest first", body = Vec<ApiKeyView>),
        (status = 401, description = "No accepted credential was presented", body = Problem),
        (status = 403, description = "That account does not administer this instance", body = Problem),
    ),
)]
pub async fn list(
    State(state): State<ApiState>,
    _caller: Administrator,
) -> AppResult<Json<Vec<ApiKeyView>>> {
    let keys = api_keys::list(state.database().readers())
        .await
        .map_err(AppError::internal)?;
    Ok(Json(keys.into_iter().map(ApiKeyView::from).collect()))
}

/// Issues a key and returns its plaintext, once.
#[utoipa::path(
    post,
    path = "/api/settings/api-keys",
    tag = "settings",
    request_body = NewApiKey,
    responses(
        (status = 200, description = "The key, with its plaintext", body = IssuedKey),
        (status = 400, description = "The name was refused", body = Problem),
        (status = 401, description = "No accepted credential was presented", body = Problem),
        (status = 403, description = "That account does not administer this instance", body = Problem),
    ),
)]
pub async fn create(
    State(state): State<ApiState>,
    Administrator(caller): Administrator,
    JsonBody(request): JsonBody<NewApiKey>,
) -> AppResult<Json<IssuedKey>> {
    let name = request.name.trim().to_owned();
    if name.is_empty() {
        return Err(AppError::new(
            Problem::new(ErrorCode::Invalid, "A name is required.").at("/name"),
        ));
    }

    let issued = IssuedApiKey::generate();
    let record = state
        .database()
        .writer()
        .submit(CreateApiKey {
            id: Id::generate(state.clock()),
            name,
            digest: issued.digest().to_owned(),
            prefix: issued.prefix().to_owned(),
            created_by: Some(caller.user_id.clone()),
            at: state.clock().now(),
        })
        .await
        .map_err(AppError::internal)?;

    Ok(Json(IssuedKey {
        key: ApiKeyView::from(record),
        secret: issued.value().to_owned(),
    }))
}

/// Revokes one key. It is refused on its next use.
#[utoipa::path(
    delete,
    path = "/api/settings/api-keys/{id}",
    tag = "settings",
    params(("id" = String, Path, description = "The key to revoke")),
    responses(
        (status = 204, description = "The key is revoked"),
        (status = 401, description = "No accepted credential was presented", body = Problem),
        (status = 403, description = "That account does not administer this instance", body = Problem),
        (status = 404, description = "No such key, or it was already revoked", body = Problem),
    ),
)]
pub async fn revoke(
    State(state): State<ApiState>,
    _caller: Administrator,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    let revoked = state
        .database()
        .writer()
        .submit(RevokeApiKey {
            id,
            at: state.clock().now(),
        })
        .await
        .map_err(AppError::internal)?;

    if revoked {
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(AppError::of(
            ErrorCode::NotFound,
            "That key does not exist, or was already revoked.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use afisharr_core::time::Timestamp;

    use super::*;

    fn record() -> ApiKeyRecord {
        ApiKeyRecord {
            id: "K".to_owned(),
            name: "Home Assistant".to_owned(),
            prefix: "0a1b2c3d".to_owned(),
            created_at: Timestamp::from_millis(1_000),
            created_by: Some("U".to_owned()),
            last_used_at: None,
            revoked_at: None,
        }
    }

    #[test]
    fn the_list_shape_carries_no_material_that_authenticates() {
        let encoded = serde_json::to_value(ApiKeyView::from(record())).expect("serialises");
        let mut keys: Vec<&str> = encoded
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "createdAt",
                "id",
                "lastUsedAt",
                "name",
                "prefix",
                "revokedAt"
            ]
        );
    }

    #[test]
    fn the_issued_shape_carries_the_plaintext_alongside_the_list_fields() {
        // A recognisably fake value: the shape under test is where the
        // plaintext sits in the body, not what a real key looks like.
        let plaintext = "x".repeat(64);
        let issued = IssuedKey {
            key: ApiKeyView::from(record()),
            secret: plaintext.clone(),
        };
        let encoded = serde_json::to_value(&issued).expect("serialises");
        assert_eq!(encoded["secret"], plaintext);
        assert_eq!(encoded["prefix"], "0a1b2c3d");
    }

    #[test]
    fn the_request_body_rejects_a_field_it_does_not_know() {
        assert!(serde_json::from_str::<NewApiKey>(r#"{"name":"a","scopes":["*"]}"#).is_err());
    }
}
