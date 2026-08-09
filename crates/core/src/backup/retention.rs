// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Naming pre-migration copies, and keeping only the newest few.

use std::path::{Path, PathBuf};

use crate::{backup::BackupError, time::Timestamp};

/// The filename prefix every automatic pre-migration copy carries.
pub const PRE_MIGRATION_PREFIX: &str = "pre-migration-";

/// The path a pre-migration copy for `version` taken at `at` is written to.
///
/// The version and the instant are both in the name so an operator choosing a
/// copy to restore can tell which schema it holds without opening it.
#[must_use]
pub fn pre_migration_path(directory: impl AsRef<Path>, version: i64, at: Timestamp) -> PathBuf {
    directory.as_ref().join(format!(
        "{PRE_MIGRATION_PREFIX}{version}-{}.db",
        at.as_millis()
    ))
}

/// Deletes all but the newest `keep` pre-migration copies, and reports what went.
///
/// Ordering is by filename, which sorts by version and then by the millisecond
/// timestamp — both zero-padded by construction only in the sense that they are
/// monotonic integers, so the comparison is numeric on the parsed parts rather
/// than lexicographic on the string.
///
/// # Errors
/// Returns [`BackupError::Directory`] when the directory cannot be listed or a
/// file cannot be removed.
pub async fn prune(directory: impl AsRef<Path>, keep: usize) -> Result<Vec<PathBuf>, BackupError> {
    let directory = directory.as_ref().to_path_buf();
    let mut copies = match tokio::fs::read_dir(&directory).await {
        Ok(entries) => collect(entries).await?,
        // Nothing has ever been backed up here. That is not a failure to prune.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(BackupError::Directory {
                path: directory,
                source,
            });
        }
    };

    copies.sort_unstable_by_key(|(key, _)| std::cmp::Reverse(*key));

    let mut removed = Vec::new();
    for (_, path) in copies.into_iter().skip(keep) {
        tokio::fs::remove_file(&path)
            .await
            .map_err(|source| BackupError::Directory {
                path: path.clone(),
                source,
            })?;
        removed.push(path);
    }
    Ok(removed)
}

/// Every pre-migration copy in the directory, keyed by `(version, taken_at)`.
async fn collect(
    mut entries: tokio::fs::ReadDir,
) -> Result<Vec<((i64, i64), PathBuf)>, BackupError> {
    let mut copies = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|source| BackupError::Directory {
            path: PathBuf::new(),
            source,
        })?
    {
        let path = entry.path();
        if let Some(key) = parse_name(&path) {
            copies.push((key, path));
        }
    }
    Ok(copies)
}

/// `(version, taken_at)` from `pre-migration-<version>-<millis>.db`, or `None`
/// when the file is something else that happens to live in the directory.
fn parse_name(path: &Path) -> Option<(i64, i64)> {
    let name = path.file_name()?.to_str()?;
    let rest = name
        .strip_prefix(PRE_MIGRATION_PREFIX)?
        .strip_suffix(".db")?;
    let (version, taken_at) = rest.split_once('-')?;
    Some((version.parse().ok()?, taken_at.parse().ok()?))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn the_name_carries_the_version_and_the_instant() {
        let path = pre_migration_path("/data/backups", 7, Timestamp::from_millis(1_700));
        assert!(path.ends_with("pre-migration-7-1700.db"));
    }

    #[test]
    fn a_file_that_is_not_a_pre_migration_copy_is_not_parsed() {
        assert!(parse_name(Path::new("/b/nightly-2026.db")).is_none());
        assert!(parse_name(Path::new("/b/pre-migration-seven-1700.db")).is_none());
    }

    #[tokio::test]
    async fn pruning_keeps_the_newest_three_and_leaves_other_files_alone() {
        let dir = TempDir::new().unwrap();
        for taken_at in 1..=5 {
            tokio::fs::write(
                pre_migration_path(dir.path(), 1, Timestamp::from_millis(taken_at)),
                [],
            )
            .await
            .unwrap();
        }
        let unrelated = dir.path().join("nightly-2026.db");
        tokio::fs::write(&unrelated, []).await.unwrap();

        let removed = prune(dir.path(), 3).await.unwrap();

        assert_eq!(removed.len(), 2, "five copies minus the three kept");
        for taken_at in 3..=5 {
            assert!(pre_migration_path(dir.path(), 1, Timestamp::from_millis(taken_at)).exists());
        }
        assert!(
            unrelated.exists(),
            "pruning must not touch files it did not write"
        );
    }

    #[tokio::test]
    async fn pruning_a_directory_that_does_not_exist_is_not_an_error() {
        let dir = TempDir::new().unwrap();
        assert!(
            prune(dir.path().join("never-used"), 3)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_higher_version_outranks_an_older_one_taken_later() {
        let dir = TempDir::new().unwrap();
        let old_schema = pre_migration_path(dir.path(), 1, Timestamp::from_millis(9_999));
        let new_schema = pre_migration_path(dir.path(), 2, Timestamp::from_millis(1));
        tokio::fs::write(&old_schema, []).await.unwrap();
        tokio::fs::write(&new_schema, []).await.unwrap();

        prune(dir.path(), 1).await.unwrap();

        assert!(new_schema.exists());
        assert!(!old_schema.exists());
    }
}
