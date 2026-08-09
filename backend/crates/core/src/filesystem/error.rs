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
}
