// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Resolve against an open handle, and never against a path string.

use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

use cap_std::{ambient_authority, fs::Dir};

use crate::filesystem::ContainmentError;

/// A directory the operator has allowed Afisharr to reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    /// The identifier a caller addresses this root by.
    ///
    /// The `asset_roots` row's own key, and never anything derived from the
    /// path. Two enabled roots can share a purpose and a final directory name
    /// — `/mnt/a/posters` and `/mnt/b/posters` — and a derived identifier makes
    /// the second of them unreachable while quietly pointing every request for
    /// it at the first.
    pub id: String,
    /// The operator's name for this root. The only thing a refusal discloses.
    ///
    /// For reading, not for addressing: it is allowed to collide, and nothing
    /// resolves a root by it.
    pub label: String,
    /// The configured path, as written. Resolved on every check.
    pub path: PathBuf,
}

impl Root {
    /// A root under `path`, addressed by `id` and shown as `label`.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            path: path.into(),
        }
    }
}

/// A path that has been proved to sit inside its root, and the handle that
/// proves it.
///
/// Constructing one is the only way to reach anything inside a root, which is
/// what makes "the check was skipped" unrepresentable rather than merely
/// discouraged. It carries an open directory and a name inside it, never an
/// absolute path: a path is a question the filesystem answers again on every
/// call, and the answer is allowed to change between two of them. Handing one
/// out is what let a checked directory be replaced before it was opened.
#[derive(Debug)]
pub struct Contained {
    root_label: String,
    /// The directory the contained path lives in, held open.
    parent: Dir,
    /// The final component, inside `parent`. `None` when the contained path is
    /// the root itself.
    name: Option<OsString>,
    /// The path relative to the root, which is what the interface displays.
    relative: PathBuf,
}

impl Contained {
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

    /// Opens the directory this path names.
    ///
    /// Handle-relative, so the directory that was checked is the directory
    /// that is opened, however the tree changes in between.
    ///
    /// Classified through [`ContainmentError::from_failed_open`], because this
    /// is the open that races: anything with write access inside the root can
    /// replace the checked name with a link out before it runs. cap-std refuses
    /// that open, and calling the refusal "could not be read" answers 404 —
    /// what a mistyped directory gets — for the escape this module catches.
    pub(crate) fn open_directory(&self) -> Result<Dir, ContainmentError> {
        let opened = match self.name.as_deref() {
            Some(name) => self.parent.open_dir(name),
            // The root itself: the handle already names it.
            None => self.parent.try_clone(),
        };
        opened.map_err(|source| ContainmentError::from_failed_open(&self.root_label, source))
    }
}

/// Resolves `requested` inside `root` and refuses anything that escapes it.
///
/// `requested` is interpreted as relative to the root, and it is walked one
/// component at a time against an open directory handle. An absolute path, a
/// traversal sequence, and a symbolic link pointing outside are all refused by
/// the same rule rather than by three special cases — `..` and a root
/// component never enter the walk, and every open is `RESOLVE_BENEATH`, which
/// refuses a link whose target is not a descendant of the handle it was found
/// in.
///
/// Handles rather than canonicalisation, because canonicalising and then
/// reopening by path is two resolutions with a gap between them: anything that
/// can write inside an enabled root can replace a checked directory in that gap
/// and have the reopen follow it out. That is `I-SEC-3` failing while every
/// path check in the codebase still passes.
///
/// The path must exist. A path that is about to be created is checked with
/// [`contain_new`], which resolves the parent through this same function.
///
/// # Errors
/// Returns [`ContainmentError::UnresolvableRoot`] when the root itself cannot
/// be opened, [`ContainmentError::Outside`] when the requested path climbs, is
/// absolute, or resolves through a link whose target is not inside the root,
/// and [`ContainmentError::Unresolvable`] when a component is simply not there.
pub fn contain(root: &Root, requested: &Path) -> Result<Contained, ContainmentError> {
    let components = inside(root, requested)?;
    let root_dir = open_root(root)?;

    let Some((name, parents)) = components.split_last() else {
        // The root itself.
        return Ok(Contained {
            root_label: root.label.clone(),
            parent: root_dir,
            name: None,
            relative: PathBuf::new(),
        });
    };

    let mut parent = root_dir;
    for component in parents {
        parent = parent
            .open_dir(component)
            .map_err(|source| ContainmentError::from_failed_open(&root.label, source))?;
    }

    // The last component is proved through the same handle rather than
    // assumed: `contain` promises the path exists, and a caller that acted on
    // the promise without it would be opening a name nothing checked.
    parent
        .metadata(name)
        .map_err(|source| ContainmentError::from_failed_open(&root.label, source))?;

    Ok(Contained {
        root_label: root.label.clone(),
        parent,
        name: Some(name.clone()),
        relative: components.iter().collect(),
    })
}

/// Resolves a path that does not exist yet, for a write.
///
/// The parent must exist and must pass [`contain`]; the final component must be
/// an ordinary name. That second rule is what stops `parent/../../escape` from
/// being handed to a writer that would create it: the parent is opened, and the
/// name appended to it cannot climb.
///
/// This is the containment path `I-SEC-4` requires for placeholder writes, and
/// it is the same function — a second implementation is failure pattern P7 in
/// the component that writes files into a user's library.
///
/// # Errors
/// Returns the same refusals as [`contain`], plus
/// [`ContainmentError::Outside`] when the final component is not an ordinary
/// name.
pub fn contain_new(root: &Root, requested: &Path) -> Result<Contained, ContainmentError> {
    let outside = || ContainmentError::Outside {
        root_label: root.label.clone(),
    };

    let name = match requested.components().next_back() {
        Some(Component::Normal(name)) => name.to_os_string(),
        _ => return Err(outside()),
    };

    let parent = requested.parent().unwrap_or_else(|| Path::new(""));
    let contained_parent = contain(root, parent)?;
    let directory = contained_parent.open_directory()?;

    Ok(Contained {
        root_label: contained_parent.root_label,
        parent: directory,
        relative: contained_parent.relative.join(&name),
        name: Some(name),
    })
}

/// The root, held open.
fn open_root(root: &Root) -> Result<Dir, ContainmentError> {
    Dir::open_ambient_dir(&root.path, ambient_authority()).map_err(|source| {
        ContainmentError::UnresolvableRoot {
            root_label: root.label.clone(),
            source,
        }
    })
}

/// The components of `requested`, refusing anything that is not a plain name.
///
/// `..` never reaches the walk. Resolving it would mean deciding where it lands
/// before opening anything, which is the string comparison this module exists
/// to not do; refusing it costs a caller nothing, because every path inside a
/// root can be written without one.
fn inside(root: &Root, requested: &Path) -> Result<Vec<OsString>, ContainmentError> {
    let mut components = Vec::new();
    for component in requested.components() {
        match component {
            Component::Normal(name) => components.push(name.to_os_string()),
            // `./here` is `here`.
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ContainmentError::Outside {
                    root_label: root.label.clone(),
                });
            }
        }
    }
    Ok(components)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use tempfile::TempDir;

    use super::*;

    fn root_with_a_file() -> (TempDir, Root) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("assets");
        std::fs::create_dir_all(root.join("posters")).unwrap();
        std::fs::write(root.join("posters").join("a.png"), b"x").unwrap();
        std::fs::write(dir.path().join("outside.txt"), b"x").unwrap();
        let jail = Root::new("01JROOT0000000000000000000", "assets", root);
        (dir, jail)
    }

    #[test]
    fn a_path_inside_the_root_resolves() {
        let (_dir, root) = root_with_a_file();
        let contained = contain(&root, Path::new("posters/a.png")).unwrap();
        assert_eq!(contained.relative(), Path::new("posters/a.png"));
        assert_eq!(contained.root_label(), "assets");
    }

    #[test]
    fn a_leading_current_directory_is_not_part_of_the_path() {
        let (_dir, root) = root_with_a_file();
        let contained = contain(&root, Path::new("./posters/./a.png")).unwrap();
        assert_eq!(contained.relative(), Path::new("posters/a.png"));
    }

    #[test]
    fn the_root_itself_resolves_to_an_empty_relative_path() {
        let (_dir, root) = root_with_a_file();
        let contained = contain(&root, Path::new("")).unwrap();
        assert_eq!(contained.relative(), Path::new(""));
        assert!(contained.open_directory().is_ok());
    }

    #[test]
    fn a_traversal_sequence_is_refused_naming_the_root() {
        let (_dir, root) = root_with_a_file();
        let error =
            contain(&root, Path::new("../outside.txt")).expect_err("traversal must be refused");
        assert_eq!(error.root_label(), Some("assets"));
        assert!(
            !error.to_string().contains("outside.txt"),
            "the message must not disclose the resolved path: {error}"
        );
    }

    #[test]
    fn a_traversal_that_would_land_back_inside_is_still_refused() {
        // `posters/..` resolves to the root, so a canonicalising check waves it
        // through. It is refused anyway: deciding where `..` lands means
        // resolving the path before opening it, which is the gap this module
        // closed.
        let (_dir, root) = root_with_a_file();
        let error =
            contain(&root, Path::new("posters/../posters")).expect_err("`..` must be refused");
        assert!(matches!(error, ContainmentError::Outside { .. }), "{error}");
    }

    #[test]
    fn an_absolute_path_is_refused() {
        let (dir, root) = root_with_a_file();
        let absolute = dir.path().join("outside.txt");
        let error = contain(&root, &absolute).expect_err("an absolute path must be refused");
        assert_eq!(error.root_label(), Some("assets"));
        assert!(matches!(error, ContainmentError::Outside { .. }), "{error}");
    }

    #[test]
    fn a_symlink_pointing_outside_the_root_is_refused() {
        let (dir, root) = root_with_a_file();
        let link = root.path.join("escape");
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.path().join("outside.txt"), &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(dir.path().join("outside.txt"), &link).unwrap();

        let error = contain(&root, Path::new("escape"))
            .expect_err("a link out of the root must be refused");
        assert_eq!(error.root_label(), Some("assets"));
        assert!(!error.to_string().contains("outside.txt"), "{error}");
        // The classification, not only the refusal: an escape reported as
        // `Unresolvable` reaches the browser as 404, which is byte-for-byte
        // the answer a mistyped directory gives.
        assert!(matches!(error, ContainmentError::Outside { .. }), "{error}");
    }

    #[test]
    fn a_relative_symlink_climbing_out_of_the_root_is_refused() {
        // The shape a canonicalising check catches too — kept because it is the
        // one an attacker writes when absolute links are refused.
        let (_dir, root) = root_with_a_file();
        let link = root.path.join("posters").join("up");
        #[cfg(unix)]
        std::os::unix::fs::symlink("../../outside.txt", &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file("..\\..\\outside.txt", &link).unwrap();

        let error = contain(&root, Path::new("posters/up"))
            .expect_err("a link out of the root must be refused");
        assert_eq!(error.root_label(), Some("assets"));
        assert!(matches!(error, ContainmentError::Outside { .. }), "{error}");
    }

    #[test]
    fn a_path_that_is_simply_not_there_stays_a_separate_answer_from_an_escape() {
        // The other half of the classification. Reporting everything unopenable
        // as an escape would answer "the path is not inside the root" to a
        // typo, which is a different lie in the same place.
        let (_dir, root) = root_with_a_file();
        for missing in ["nowhere", "posters/nothing.png"] {
            let error = contain(&root, Path::new(missing)).expect_err("must be refused");
            assert!(
                matches!(error, ContainmentError::Unresolvable { .. }),
                "{missing}: {error}"
            );
        }
    }

    #[test]
    fn a_symlink_staying_inside_the_root_resolves() {
        let (_dir, root) = root_with_a_file();
        let link = root.path.join("shortcut");
        #[cfg(unix)]
        std::os::unix::fs::symlink("posters", &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir("posters", &link).unwrap();

        let contained = contain(&root, Path::new("shortcut/a.png")).unwrap();
        // The path the caller asked for, not the one it resolved to: the
        // resolution lives in a handle now, and reporting the target would
        // disclose a layout the refusal messages are careful to hide.
        assert_eq!(contained.relative(), Path::new("shortcut/a.png"));
    }

    #[test]
    fn a_new_file_under_an_existing_directory_is_contained() {
        let (_dir, root) = root_with_a_file();
        let contained = contain_new(&root, Path::new("posters/new.png")).unwrap();
        assert_eq!(contained.relative(), Path::new("posters/new.png"));
    }

    #[test]
    fn a_new_file_whose_name_climbs_is_refused() {
        let (_dir, root) = root_with_a_file();
        for climbing in ["posters/..", "..", "posters/../../escaped"] {
            let error = contain_new(&root, Path::new(climbing))
                .expect_err("a climbing name must be refused");
            assert_eq!(error.root_label(), Some("assets"), "{climbing}");
        }
    }

    #[test]
    fn an_unresolvable_root_is_refused_before_the_path_is_examined() {
        let root = Root::new(
            "01JROOTMISSING000000000000",
            "missing",
            "/definitely/not/a/directory/afisharr",
        );
        let error = contain(&root, Path::new("anything"))
            .expect_err("an unresolvable root must be refused");
        assert!(matches!(error, ContainmentError::UnresolvableRoot { .. }));
    }
}
