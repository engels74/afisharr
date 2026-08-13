// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading one contained directory.

use std::path::{Path, PathBuf};

use cap_std::fs::Metadata;

use crate::filesystem::{ContainmentError, Root, contain};

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
/// The directory is opened once, through [`contain`], and every entry is then
/// read from *that handle* — not from a path built out of its name. The
/// difference is the whole of `I-SEC-3` under a hostile tree: a path is a
/// question the filesystem answers again on every call, so a directory checked
/// by path and reopened by path can be replaced in between by anything that can
/// write inside the root, and the reopen follows the replacement out. A handle
/// names the directory that was checked and keeps naming it.
///
/// An entry whose metadata cannot be read through the handle — a link out of
/// the root is exactly this — is omitted rather than reported: the browser's
/// job is to show what the operator may reach, and a refused entry named in the
/// listing would disclose the link's target.
///
/// # Errors
/// Returns [`ContainmentError::Outside`] when the requested path escapes the
/// root — including when it is replaced by a link out between the check and the
/// open — [`ContainmentError::Unresolvable`] when a component is not there, and
/// [`ContainmentError::Unreadable`] when the resolved directory cannot be read.
pub async fn list(root: &Root, requested: &str) -> Result<Vec<Entry>, ContainmentError> {
    let root_label = root.label.clone();
    let root = root.clone();
    let requested = PathBuf::from(requested);

    // Off the runtime's workers: cap-std is synchronous, and a directory read
    // on a network mount can block for a long time on a thread that has other
    // requests waiting on it.
    tokio::task::spawn_blocking(move || read(&root, &requested))
        .await
        .map_err(|source| ContainmentError::Unreadable {
            root_label,
            source: std::io::Error::other(source),
        })?
}

fn read(root: &Root, requested: &Path) -> Result<Vec<Entry>, ContainmentError> {
    let contained = contain(root, requested)?;
    let directory = contained.open_directory()?;
    let base = contained.relative().to_path_buf();

    let unreadable = |source: std::io::Error| ContainmentError::Unreadable {
        root_label: root.label.clone(),
        source,
    };

    let mut entries = Vec::new();
    for entry in directory.entries().map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            // A name that is not UTF-8 cannot be round-tripped through the
            // JSON the interface reads, and offering one the caller cannot
            // send back is worse than omitting it.
            continue;
        };
        let Ok(metadata) = directory.metadata(&name) else {
            // A link out of the root cannot be followed from this handle at
            // all. Omitted rather than reported: the browser's job is to show
            // what the operator may reach, and an entry named beside a refusal
            // would disclose that its target is outside.
            continue;
        };
        entries.push(describe(&base, name, &metadata));
    }

    entries.sort_by(|left, right| {
        directories_first(left.kind)
            .cmp(&directories_first(right.kind))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(entries)
}

/// Describes one entry from metadata already read through the directory's own
/// handle.
fn describe(base: &Path, name: String, metadata: &Metadata) -> Entry {
    let kind = if metadata.is_dir() {
        EntryKind::Directory
    } else if metadata.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    };
    Entry {
        relative_path: base.join(&name).to_string_lossy().into_owned(),
        name,
        size_bytes: match kind {
            EntryKind::File => Some(metadata.len()),
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

    fn root(dir: &TempDir) -> Root {
        Root::new(
            "01JROOT0000000000000000000",
            "assets",
            dir.path().join("assets"),
        )
    }

    #[tokio::test]
    async fn directories_sort_before_files_and_names_sort_within_each() {
        let dir = TempDir::new().unwrap();
        let assets = dir.path().join("assets");
        std::fs::create_dir_all(assets.join("zed")).unwrap();
        std::fs::create_dir_all(assets.join("alpha")).unwrap();
        std::fs::write(assets.join("b.png"), b"xx").unwrap();
        std::fs::write(assets.join("a.png"), b"x").unwrap();

        let entries = list(&root(&dir), "").await.unwrap();
        let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, ["alpha", "zed", "a.png", "b.png"]);
        assert_eq!(entries[2].size_bytes, Some(1));
        assert_eq!(entries[0].size_bytes, None);
    }

    #[tokio::test]
    async fn an_entry_reports_the_path_a_caller_asks_for_next() {
        let dir = TempDir::new().unwrap();
        let assets = dir.path().join("assets");
        std::fs::create_dir_all(assets.join("posters")).unwrap();
        std::fs::write(assets.join("posters").join("a.png"), b"x").unwrap();

        let entries = list(&root(&dir), "posters").await.unwrap();
        assert_eq!(entries[0].relative_path, "posters/a.png");
    }

    #[tokio::test]
    async fn a_link_out_of_the_root_is_omitted_rather_than_named() {
        let dir = TempDir::new().unwrap();
        let assets = dir.path().join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(dir.path().join("secret.txt"), b"x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.path().join("secret.txt"), assets.join("escape")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(dir.path().join("secret.txt"), assets.join("escape"))
            .unwrap();

        let entries = list(&root(&dir), "").await.unwrap();
        assert!(entries.is_empty(), "{entries:?}");
    }

    #[tokio::test]
    async fn listing_a_path_outside_the_root_is_refused() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("assets")).unwrap();

        let error = list(&root(&dir), "..")
            .await
            .expect_err("traversal must be refused");
        assert_eq!(error.root_label(), Some("assets"));
    }

    #[tokio::test]
    async fn a_directory_replaced_after_the_check_is_not_followed_out_of_the_root() {
        // The race the handle closes. `swap` is checked as a directory inside
        // the root, and then becomes a link to somewhere outside it before the
        // listing reads anything. A path-based reopen follows the link; a
        // handle keeps naming the directory that was checked.
        let dir = TempDir::new().unwrap();
        let assets = dir.path().join("assets");
        std::fs::create_dir_all(assets.join("swap")).unwrap();
        std::fs::write(assets.join("swap").join("inside.png"), b"x").unwrap();
        std::fs::create_dir_all(dir.path().join("elsewhere")).unwrap();
        std::fs::write(dir.path().join("elsewhere").join("secret.txt"), b"x").unwrap();

        let root = root(&dir);
        let contained = contain(&root, Path::new("swap")).unwrap();
        let directory = contained.open_directory().unwrap();

        // Moved aside rather than deleted, so the directory that was checked
        // still holds what it held: the question is which of the two the
        // listing reads, not whether either still exists.
        std::fs::rename(assets.join("swap"), dir.path().join("moved")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.path().join("elsewhere"), assets.join("swap")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(dir.path().join("elsewhere"), assets.join("swap"))
            .unwrap();

        let names: Vec<String> = directory
            .entries()
            .unwrap()
            .filter_map(|entry| Some(entry.ok()?.file_name().to_string_lossy().into_owned()))
            .collect();
        assert_eq!(
            names,
            vec!["inside.png".to_owned()],
            "the handle must still name the directory that was checked"
        );
        assert!(
            !names.contains(&"secret.txt".to_owned()),
            "the replacement must not have been followed out of the root"
        );
    }
}
