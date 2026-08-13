// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `/api/files`.

// Route handlers in this file document their failures in their
// `#[utoipa::path(responses(...))]` block: that block is the contract the
// generated TypeScript client is built from, and it is machine-checked. A prose
// `# Errors` section beside it would be a second statement of the same facts,
// free to drift, with nothing checking it (§24.5).
#![allow(clippy::missing_errors_doc)]

use afisharr_core::filesystem::{ContainmentError, Entry, EntryKind, Root, enabled_roots, list};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    authentication::{Administrator, FilesRead},
    error::{AppError, AppResult, ErrorCode, Problem, QueryParams},
    state::ApiState,
};

/// Which root, and where inside it.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowseQuery {
    /// The identifier of the root to walk.
    pub root: String,
    /// The path inside the root. Empty means the root itself.
    #[serde(default)]
    pub path: String,
}

/// One root the operator has allowed.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RootView {
    /// The identifier the browser addresses it by.
    ///
    /// The row's own key rather than the label: two roots of one purpose can
    /// share a final directory name, so a label is allowed to collide and
    /// addressing by it would make the second of them unreachable.
    pub id: String,
    /// The operator's name for it, which is what a refusal reads out.
    pub label: String,
}

/// One entry in a browsed directory.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    /// The entry's own name.
    pub name: String,
    /// The path relative to the root, which is what a caller asks for next.
    ///
    /// Relative and never absolute: an absolute path would tell a caller where
    /// the root lives, which is the one thing the refusal message is careful
    /// not to disclose.
    pub path: String,
    /// Whether it can be descended into.
    pub is_directory: bool,
    /// Size in bytes, for a file.
    pub size_bytes: Option<u64>,
}

impl From<Entry> for EntryView {
    fn from(entry: Entry) -> Self {
        Self {
            name: entry.name,
            path: entry.relative_path,
            is_directory: matches!(entry.kind, EntryKind::Directory),
            size_bytes: entry.size_bytes,
        }
    }
}

/// One directory's contents.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListing {
    /// The root that was walked.
    pub root: String,
    /// The path inside it.
    pub path: String,
    /// The entries, directories first and then by name.
    pub entries: Vec<EntryView>,
}

/// Lists the roots the operator has enabled.
#[utoipa::path(
    get,
    path = "/api/files/roots",
    tag = "files",
    responses(
        (status = 200, description = "Every enabled root", body = Vec<RootView>),
        (status = 401, description = "No accepted credential was presented", body = Problem),
        (status = 403, description = "That account does not administer this instance, that key was not issued with the scope this route needs, or setup has not been completed", body = Problem),
        (status = 429, description = "Too many requests", body = Problem),
    ),
)]
pub async fn roots(
    State(state): State<ApiState>,
    _caller: Administrator<FilesRead>,
) -> AppResult<Json<Vec<RootView>>> {
    Ok(Json(
        enabled(&state)
            .await?
            .into_iter()
            .map(|root| RootView {
                id: root.id,
                label: root.label,
            })
            .collect(),
    ))
}

/// Lists one directory inside one root.
///
/// Every refusal names the root and never the resolved path (`I-SEC-3`).
#[utoipa::path(
    get,
    path = "/api/files",
    tag = "files",
    params(BrowseQuery),
    responses(
        (status = 200, description = "The directory's contents", body = DirectoryListing),
        (status = 400, description = "The query was not readable", body = Problem),
        (status = 401, description = "No accepted credential was presented", body = Problem),
        (status = 403, description = "The path is not inside the root, that account does not administer this instance, that key was not issued with the scope this route needs, or setup has not been completed", body = Problem),
        (status = 404, description = "No such root", body = Problem),
        (status = 429, description = "Too many requests", body = Problem),
    ),
)]
pub async fn browse(
    State(state): State<ApiState>,
    _caller: Administrator<FilesRead>,
    QueryParams(query): QueryParams<BrowseQuery>,
) -> AppResult<Json<DirectoryListing>> {
    let root = enabled(&state)
        .await?
        .into_iter()
        .find(|root| root.id == query.root)
        .ok_or_else(|| {
            AppError::new(
                Problem::new(
                    ErrorCode::NotFound,
                    "No such filesystem root is configured.",
                )
                .at("/root"),
            )
        })?;

    let entries = list(&root, &query.path).await.map_err(refusal)?;

    Ok(Json(DirectoryListing {
        root: query.root,
        path: query.path,
        entries: entries.into_iter().map(EntryView::from).collect(),
    }))
}

/// The roots the operator has enabled, read on every call.
///
/// From the table and never from a snapshot taken when the process started: the
/// operator adds and removes roots from the interface, and a list fixed at boot
/// answered `404 No such filesystem root is configured` for a root the database
/// said was enabled, while going on offering one they had just disabled — until
/// the container was restarted. Two reader-pool queries a browse is the price,
/// and the browser is an administrator-only surface behind a rate limit.
async fn enabled(state: &ApiState) -> AppResult<Vec<Root>> {
    enabled_roots(state.database().readers())
        .await
        .map_err(AppError::internal)
}

/// Turns a containment refusal into the one shape this surface answers with.
///
/// The message comes from the error, which names the root and nothing else. An
/// `AppError` built here from the requested path would undo that care in the
/// layer above the one that took it.
fn refusal(error: ContainmentError) -> AppError {
    let code = match error {
        ContainmentError::Outside { .. } => ErrorCode::Forbidden,
        // Everything else answers `notFound`, including the refusals a later
        // release adds: the type is `#[non_exhaustive]`, and "the path is not
        // there" discloses nothing and denies access, which is the safe
        // direction (P2).
        _ => ErrorCode::NotFound,
    };
    let message = error.to_string();
    AppError::new(Problem::new(code, message).at("/path")).caused_by(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_traversal_refusal_names_the_root_and_not_the_resolved_path() {
        let error = refusal(ContainmentError::Outside {
            root_label: "assets".to_owned(),
        });
        assert_eq!(error.problem().code, ErrorCode::Forbidden);
        assert!(error.problem().message.contains("assets"));
        assert!(!error.problem().message.contains('/'));
    }

    #[test]
    fn an_entry_is_reported_by_its_relative_path() {
        let view = EntryView::from(Entry {
            name: "a.png".to_owned(),
            relative_path: "posters/a.png".to_owned(),
            kind: EntryKind::File,
            size_bytes: Some(12),
        });
        let encoded = serde_json::to_value(&view).expect("serialises");
        assert_eq!(encoded["path"], "posters/a.png");
        assert_eq!(encoded["isDirectory"], false);
        assert_eq!(encoded["sizeBytes"], 12);
    }

    #[test]
    fn a_directory_reports_no_size() {
        let view = EntryView::from(Entry {
            name: "posters".to_owned(),
            relative_path: "posters".to_owned(),
            kind: EntryKind::Directory,
            size_bytes: None,
        });
        assert!(view.is_directory);
        assert_eq!(view.size_bytes, None);
    }

    #[test]
    fn the_query_rejects_a_parameter_it_does_not_know() {
        assert!(
            serde_urlencoded::from_str::<BrowseQuery>("root=assets&absolute=true").is_err(),
            "an unknown parameter must be refused"
        );
    }
}
