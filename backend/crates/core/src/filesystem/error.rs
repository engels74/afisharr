// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Why a path was refused.

use thiserror::Error;

/// A path that is not usable inside a configured root.
///
/// Every refusal names the **root**, never the resolved path (PRD §21.4.6).
/// Naming the resolution turns the boundary into an oracle: a caller who is
/// told that `../../etc/shadow` resolved to `/etc/shadow` has learned the
/// filesystem layout from the component whose job was to hide it.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ContainmentError {
    /// The requested path resolved outside every enabled root.
    #[error("the requested path is not inside the configured root '{root_label}'")]
    Outside {
        /// The operator's name for the root the request was made against.
        root_label: String,
    },

    /// No root is configured or enabled, so nothing can be browsed.
    #[error("no filesystem root is configured")]
    NoRoot,

    /// The root itself could not be resolved — it may not exist.
    #[error("the configured root '{root_label}' could not be resolved")]
    UnresolvableRoot {
        /// The operator's name for the root.
        root_label: String,
        /// The underlying failure.
        #[source]
        source: std::io::Error,
    },

    /// The requested path could not be resolved inside the root.
    ///
    /// Carries the root's label and not the path, for the reason above.
    #[error("the requested path could not be resolved inside '{root_label}'")]
    Unresolvable {
        /// The operator's name for the root.
        root_label: String,
        /// The underlying failure.
        #[source]
        source: std::io::Error,
    },

    /// The resolved path could not be listed.
    #[error("the requested path inside '{root_label}' could not be read")]
    Unreadable {
        /// The operator's name for the root.
        root_label: String,
        /// The underlying failure.
        #[source]
        source: std::io::Error,
    },
}

impl ContainmentError {
    /// The root this refusal is about, when it names one.
    #[must_use]
    pub fn root_label(&self) -> Option<&str> {
        match self {
            Self::Outside { root_label }
            | Self::UnresolvableRoot { root_label, .. }
            | Self::Unresolvable { root_label, .. }
            | Self::Unreadable { root_label, .. } => Some(root_label),
            Self::NoRoot => None,
        }
    }

    /// Classifies one failed open inside a root.
    ///
    /// Two very different answers wear one `io::Error` here, and telling them
    /// apart is the whole of this constructor. A component that is simply not
    /// there is a typo or a directory that has been moved, and the honest
    /// answer is "no such path". A component that *is* there and leads out of
    /// the root is a refusal: the operator has a symbolic link into a second
    /// mount — an ordinary media layout — and what they need to be told is that
    /// its target sits outside every enabled root, not that it does not exist.
    /// Answering "not found" to both leaves them unable to tell a mistyped name
    /// from a directory they have to add as a root, and it is the
    /// classification the canonicalise-then-compare jail gave before the walk
    /// moved onto handles.
    #[must_use]
    pub(crate) fn from_failed_open(root_label: &str, source: std::io::Error) -> Self {
        if escaped(&source) {
            return Self::Outside {
                root_label: root_label.to_owned(),
            };
        }
        Self::Unresolvable {
            root_label: root_label.to_owned(),
            source,
        }
    }
}

/// Whether cap-std refused an open because the path led out of the root.
///
/// The signature is cap-std's own: every escape it refuses is built by
/// `escape_attempt()`, an `io::Error` constructed in-process with
/// `PermissionDenied` and therefore carrying no `raw_os_error`. A real `EACCES`
/// from the kernel carries one, so a directory the process genuinely may not
/// read stays [`ContainmentError::Unresolvable`] rather than being reported as
/// an escape it is not.
fn escaped(source: &std::io::Error) -> bool {
    source.kind() == std::io::ErrorKind::PermissionDenied && source.raw_os_error().is_none()
}
