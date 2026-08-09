// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading one contained directory.

use crate::filesystem::{Contained, ContainmentError, Root, contain};

/// What one entry in a browsed directory is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A directory the operator can descend into.
    Directory,
    /// An ordinary file.
    File,
    /// Something that is neither, and that the browser will not open.
    Other,
}

/// One entry in a browsed directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The entry's own name, with no path attached.
    pub name: String,
    /// The path relative to the root, which is what a caller asks for next.
    pub relative_path: String,
    /// What the entry is.
    pub kind: EntryKind,
    /// Size in bytes, for a file.
    pub size_bytes: Option<u64>,
}

/// Lists the directory `requested` names inside `root`, sorted.
///
/// Every entry is re-checked through [`contain`], so a link that appeared
/// between the directory read and this listing cannot smuggle an outside path
/// into the result. Entries that fail the check are omitted rather than
/// reported: the browser's job is to show what the operator may reach, and a
/// refused entry named in the listing would disclose the link's target.
///
/// # Errors
/// Returns [`ContainmentError::Outside`] when the requested path escapes the
/// root, and [`ContainmentError::Unreadable`] when the resolved directory
/// cannot be read.
pub async fn list(root: &Root, requested: &str) -> Result<Vec<Entry>, ContainmentError> {
    let directory = contain(root, std::path::Path::new(requested)).await?;

    let mut reader = tokio::fs::read_dir(directory.absolute())
        .await
        .map_err(|source| ContainmentError::Unreadable {
            root_label: root.label.clone(),
            source,
        })?;

    let mut entries = Vec::new();
    while let Some(entry) =
        reader
            .next_entry()
            .await
            .map_err(|source| ContainmentError::Unreadable {
                root_label: root.label.clone(),
                source,
            })?
    {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            // A name that is not UTF-8 cannot be round-tripped through the
            // JSON the interface reads, and offering one the caller cannot
            // send back is worse than omitting it.
            continue;
        };
        let child = directory.relative().join(&name);
        let Ok(contained) = contain(root, &child).await else {
            continue;
        };
        entries.push(describe(&name, &contained).await);
    }

    entries.sort_by(|left, right| {
        directories_first(left.kind)
            .cmp(&directories_first(right.kind))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(entries)
}

async fn describe(name: &str, contained: &Contained) -> Entry {
    let metadata = tokio::fs::metadata(contained.absolute()).await.ok();
    let kind = match metadata.as_ref().map(std::fs::Metadata::file_type) {
        Some(file_type) if file_type.is_dir() => EntryKind::Directory,
        Some(file_type) if file_type.is_file() => EntryKind::File,
        _ => EntryKind::Other,
    };
    Entry {
        name: name.to_owned(),
        relative_path: contained.relative().to_string_lossy().into_owned(),
        size_bytes: match kind {
            EntryKind::File => metadata.map(|metadata| metadata.len()),
            EntryKind::Directory | EntryKind::Other => None,
        },
        kind,
    }
}

const fn directories_first(kind: EntryKind) -> u8 {
    match kind {
        EntryKind::Directory => 0,
        EntryKind::File => 1,
        EntryKind::Other => 2,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn directories_sort_before_files_and_names_sort_within_each() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("assets");
        tokio::fs::create_dir_all(root.join("zed")).await.unwrap();
        tokio::fs::create_dir_all(root.join("alpha")).await.unwrap();
        tokio::fs::write(root.join("b.png"), b"xx").await.unwrap();
        tokio::fs::write(root.join("a.png"), b"x").await.unwrap();

        let entries = list(&Root::new("assets", root), "").await.unwrap();
        let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, ["alpha", "zed", "a.png", "b.png"]);
        assert_eq!(entries[2].size_bytes, Some(1));
        assert_eq!(entries[0].size_bytes, None);
    }

    #[tokio::test]
    async fn a_link_out_of_the_root_is_omitted_rather_than_named() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("assets");
        tokio::fs::create_dir_all(&root).await.unwrap();
        tokio::fs::write(dir.path().join("secret.txt"), b"x")
            .await
            .unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.path().join("secret.txt"), root.join("escape")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(dir.path().join("secret.txt"), root.join("escape"))
            .unwrap();

        let entries = list(&Root::new("assets", root), "").await.unwrap();
        assert!(entries.is_empty(), "{entries:?}");
    }

    #[tokio::test]
    async fn listing_a_path_outside_the_root_is_refused() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("assets");
        tokio::fs::create_dir_all(&root).await.unwrap();

        let error = list(&Root::new("assets", root), "..")
            .await
            .expect_err("traversal must be refused");
        assert_eq!(error.root_label(), Some("assets"));
    }
}
