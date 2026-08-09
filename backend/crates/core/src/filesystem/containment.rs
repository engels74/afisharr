// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Resolve first, then check containment. Never the other way round.

use std::path::{Component, Path, PathBuf};

use crate::filesystem::ContainmentError;

/// A directory the operator has allowed Afisharr to reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    /// The operator's name for this root. The only thing a refusal discloses.
    pub label: String,
    /// The configured path, as written. Resolved on every check.
    pub path: PathBuf,
}

impl Root {
    /// A root under `path`, labelled `label`.
    #[must_use]
    pub fn new(label: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            label: label.into(),
            path: path.into(),
        }
    }
}

/// A path that has been resolved and proved to sit inside its root.
///
/// Constructing one is the only way to get an absolute path out of this
/// module, which is what makes "the check was skipped" unrepresentable rather
/// than merely discouraged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contained {
    root_label: String,
    absolute: PathBuf,
    relative: PathBuf,
}

impl Contained {
    /// The resolved absolute path. Safe to open.
    #[must_use]
    pub fn absolute(&self) -> &Path {
        &self.absolute
    }

    /// The path relative to the root, which is what the interface displays.
    #[must_use]
    pub fn relative(&self) -> &Path {
        &self.relative
    }

    /// The root this path was proved to sit inside.
    #[must_use]
    pub fn root_label(&self) -> &str {
        &self.root_label
    }
}

/// Resolves `requested` inside `root` and refuses anything that escapes it.
///
/// `requested` is interpreted as relative to the root. An absolute path, a
/// traversal sequence, and a symbolic link pointing outside are all refused by
/// the same rule rather than by three special cases: the join is canonicalised
/// — which resolves `..`, `.`, and every link on the way — and only then is the
/// result tested for containment. Checking a prefix before resolution is the
/// classic mistake `I-SEC-3` names, because `roots/../../etc` passes it.
///
/// The path must exist. A path that is about to be created is checked with
/// [`contain_new`], which resolves the parent through this same function.
///
/// # Errors
/// Returns [`ContainmentError::UnresolvableRoot`] when the root itself cannot
/// be resolved, [`ContainmentError::Unresolvable`] when the requested path
/// cannot, and [`ContainmentError::Outside`] when the resolved path is not
/// under the resolved root.
pub async fn contain(root: &Root, requested: &Path) -> Result<Contained, ContainmentError> {
    let resolved_root = tokio::fs::canonicalize(&root.path)
        .await
        .map_err(|source| ContainmentError::UnresolvableRoot {
            root_label: root.label.clone(),
            source,
        })?;

    let candidate = resolved_root.join(requested);
    let resolved = tokio::fs::canonicalize(&candidate)
        .await
        .map_err(|source| ContainmentError::Unresolvable {
            root_label: root.label.clone(),
            source,
        })?;

    let relative = resolved
        .strip_prefix(&resolved_root)
        .map_err(|_| ContainmentError::Outside {
            root_label: root.label.clone(),
        })?
        .to_path_buf();

    Ok(Contained {
        root_label: root.label.clone(),
        absolute: resolved,
        relative,
    })
}

/// Resolves a path that does not exist yet, for a write.
///
/// The parent must exist and must pass [`contain`]; the final component must be
/// an ordinary name. That second rule is what stops `parent/../../escape` from
/// being handed to a writer that would create it: the parent is resolved, and
/// the name appended to it cannot climb.
///
/// This is the containment path `I-SEC-4` requires for placeholder writes, and
/// it is the same function — a second implementation is failure pattern P7 in
/// the component that writes files into a user's library.
///
/// # Errors
/// Returns the same refusals as [`contain`], plus
/// [`ContainmentError::Outside`] when the final component is not an ordinary
/// name.
pub async fn contain_new(root: &Root, requested: &Path) -> Result<Contained, ContainmentError> {
    let outside = || ContainmentError::Outside {
        root_label: root.label.clone(),
    };

    let name = match requested.components().next_back() {
        Some(Component::Normal(name)) => name.to_os_string(),
        _ => return Err(outside()),
    };

    let parent = requested.parent().unwrap_or_else(|| Path::new(""));
    let contained_parent = contain(root, parent).await?;

    Ok(Contained {
        root_label: contained_parent.root_label,
        absolute: contained_parent.absolute.join(&name),
        relative: contained_parent.relative.join(&name),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use tempfile::TempDir;

    use super::*;

    async fn root_with_a_file() -> (TempDir, Root) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("assets");
        tokio::fs::create_dir_all(root.join("posters"))
            .await
            .unwrap();
        tokio::fs::write(root.join("posters").join("a.png"), b"x")
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("outside.txt"), b"x")
            .await
            .unwrap();
        let jail = Root::new("assets", root);
        (dir, jail)
    }

    #[tokio::test]
    async fn a_path_inside_the_root_resolves() {
        let (_dir, root) = root_with_a_file().await;
        let contained = contain(&root, Path::new("posters/a.png")).await.unwrap();
        assert_eq!(contained.relative(), Path::new("posters/a.png"));
        assert_eq!(contained.root_label(), "assets");
    }

    #[tokio::test]
    async fn a_traversal_sequence_is_refused_naming_the_root() {
        let (_dir, root) = root_with_a_file().await;
        let error = contain(&root, Path::new("../outside.txt"))
            .await
            .expect_err("traversal must be refused");
        assert_eq!(error.root_label(), Some("assets"));
        assert!(
            !error.to_string().contains("outside.txt"),
            "the message must not disclose the resolved path: {error}"
        );
    }

    #[tokio::test]
    async fn an_absolute_path_is_refused() {
        let (dir, root) = root_with_a_file().await;
        let absolute = dir.path().join("outside.txt");
        let error = contain(&root, &absolute)
            .await
            .expect_err("an absolute path must be refused");
        assert_eq!(error.root_label(), Some("assets"));
    }

    #[tokio::test]
    async fn a_symlink_pointing_outside_the_root_is_refused() {
        let (dir, root) = root_with_a_file().await;
        let link = root.path.join("escape");
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.path().join("outside.txt"), &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(dir.path().join("outside.txt"), &link).unwrap();

        let error = contain(&root, Path::new("escape"))
            .await
            .expect_err("a link out of the root must be refused");
        assert_eq!(error.root_label(), Some("assets"));
        assert!(!error.to_string().contains("outside.txt"), "{error}");
    }

    #[tokio::test]
    async fn a_symlink_staying_inside_the_root_resolves() {
        let (_dir, root) = root_with_a_file().await;
        let link = root.path.join("shortcut");
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.path.join("posters"), &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(root.path.join("posters"), &link).unwrap();

        let contained = contain(&root, Path::new("shortcut/a.png")).await.unwrap();
        assert_eq!(contained.relative(), Path::new("posters/a.png"));
    }

    #[tokio::test]
    async fn a_new_file_under_an_existing_directory_is_contained() {
        let (_dir, root) = root_with_a_file().await;
        let contained = contain_new(&root, Path::new("posters/new.png"))
            .await
            .unwrap();
        assert_eq!(contained.relative(), Path::new("posters/new.png"));
        assert!(!contained.absolute().exists());
    }

    #[tokio::test]
    async fn a_new_file_whose_name_climbs_is_refused() {
        let (_dir, root) = root_with_a_file().await;
        for climbing in ["posters/..", "..", "posters/../../escaped"] {
            let error = contain_new(&root, Path::new(climbing))
                .await
                .expect_err("a climbing name must be refused");
            assert_eq!(error.root_label(), Some("assets"), "{climbing}");
        }
    }

    #[tokio::test]
    async fn an_unresolvable_root_is_refused_before_the_path_is_examined() {
        let root = Root::new("missing", "/definitely/not/a/directory/afisharr");
        let error = contain(&root, Path::new("anything"))
            .await
            .expect_err("an unresolvable root must be refused");
        assert!(matches!(error, ContainmentError::UnresolvableRoot { .. }));
    }
}
