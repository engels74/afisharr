// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `asset_roots` table: what the operator has allowed Afisharr to reach.

use sqlx::SqlitePool;

use crate::filesystem::Root;

/// Every enabled root, ready to browse.
///
/// A disabled row is not returned at all rather than returned with a flag: the
/// browser's whole contract is that it can only reach what is in the list, and
/// a list holding entries the caller must remember not to use is one entry away
/// from a traversal that was allowed by accident.
///
/// A root is addressed by its row's own key and *read* by a label, and the two
/// are separate on purpose. The label is the row's `purpose` plus its path's
/// final component, which is what makes a refusal legible without naming the
/// path — and which two roots can share: `/mnt/a/posters` and `/mnt/b/posters`
/// under one purpose produce one label between them. Addressing by that label
/// would resolve both to whichever row came back first, leaving the other
/// unreachable and every request for it pointed somewhere the caller did not
/// ask for.
///
/// # Errors
/// Returns the underlying `sqlx` failure.
pub async fn enabled(readers: &SqlitePool) -> Result<Vec<Root>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT id, path, purpose FROM asset_roots WHERE is_enabled = 1 ORDER BY purpose, path"
    )
    .fetch_all(readers)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Root::new(row.id, label_for(&row.purpose, &row.path), row.path))
        .collect())
}

/// The label a root is read by.
fn label_for(purpose: &str, path: &str) -> String {
    let leaf = path
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path);
    if leaf.is_empty() {
        purpose.to_owned()
    } else {
        format!("{purpose}/{leaf}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_names_the_purpose_and_the_final_component() {
        assert_eq!(label_for("Browse", "/media/posters"), "Browse/posters");
    }

    #[test]
    fn two_roots_of_one_purpose_get_different_labels() {
        assert_ne!(
            label_for("LocalPosters", "/media/films"),
            label_for("LocalPosters", "/media/shows")
        );
    }

    #[test]
    fn a_trailing_separator_does_not_produce_an_empty_label() {
        assert_eq!(label_for("Fonts", "/usr/share/fonts/"), "Fonts/fonts");
    }

    #[test]
    fn a_root_at_the_filesystem_root_falls_back_to_its_purpose() {
        assert_eq!(label_for("Browse", "/"), "Browse");
    }

    #[test]
    fn two_roots_that_share_a_final_component_are_still_addressed_apart() {
        // The label is allowed to collide — it is what a refusal reads out —
        // so the identifier must not be derived from it. Both of these are
        // `Browse/posters`, and browsing either must reach the one asked for.
        let first = Root::new(
            "01JROOTA0000000000000000000",
            label_for("Browse", "/mnt/a/posters"),
            "/mnt/a/posters",
        );
        let second = Root::new(
            "01JROOTB0000000000000000000",
            label_for("Browse", "/mnt/b/posters"),
            "/mnt/b/posters",
        );
        assert_eq!(first.label, second.label);
        assert_ne!(first.id, second.id);
    }
}
