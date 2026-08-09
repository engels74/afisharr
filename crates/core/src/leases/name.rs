// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The hierarchical names a lease may carry, and who holds one.

use std::fmt;

/// The scope a lease covers.
///
/// An enum rather than a free string: PRD §19.4 fixes the hierarchy, and a
/// mistyped `pass:placement` that silently serialises nothing is exactly the
/// failure the lease exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LeaseName {
    /// One collection definition's reconciliation pass.
    CollectionPass {
        /// The definition being reconciled.
        definition_id: String,
    },
    /// One library's placement pass. Placement is serialised per library.
    PlacementPass {
        /// The library whose ordering space is being planned.
        library_id: String,
    },
    /// One library's lifecycle pass.
    LifecyclePass {
        /// The library whose subjects are being evaluated.
        library_id: String,
    },
    /// One scheduled or manually triggered job run.
    Job {
        /// The job being run.
        job_id: String,
    },
    /// The setup wizard's exclusive claim, held by a browser rather than a task.
    SetupClaim,
}

impl LeaseName {
    /// The text stored in `leases.name`.
    #[must_use]
    pub fn as_text(&self) -> String {
        match self {
            Self::CollectionPass { definition_id } => format!("pass:collection:{definition_id}"),
            Self::PlacementPass { library_id } => format!("pass:placement:{library_id}"),
            Self::LifecyclePass { library_id } => format!("pass:lifecycle:{library_id}"),
            Self::Job { job_id } => format!("job:{job_id}"),
            Self::SetupClaim => "setup:claim".to_owned(),
        }
    }
}

impl fmt::Display for LeaseName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_text())
    }
}

/// Who holds a lease.
///
/// For a pass this is the process instance plus the task within it, so startup
/// can tell its own abandoned leases from another process's live ones. For
/// [`LeaseName::SetupClaim`] it is the SHA-256 of the claim cookie, which is why
/// startup's "clear leases owned by this process" step never matches it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LeaseOwner(String);

impl LeaseOwner {
    /// An owner naming this process instance and the task inside it.
    #[must_use]
    pub fn task(instance_id: &str, task: &str) -> Self {
        Self(format!("{instance_id}/{task}"))
    }

    /// An owner that is an opaque token rather than a task — the setup claim.
    #[must_use]
    pub fn token(digest: &str) -> Self {
        Self(digest.to_owned())
    }

    /// The prefix that matches every lease this process instance owns.
    #[must_use]
    pub fn instance_prefix(instance_id: &str) -> String {
        format!("{instance_id}/")
    }

    /// The text stored in `leases.owner`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LeaseOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_renders_its_documented_hierarchy() {
        let cases = [
            (
                LeaseName::CollectionPass {
                    definition_id: "D1".into(),
                },
                "pass:collection:D1",
            ),
            (
                LeaseName::PlacementPass {
                    library_id: "L1".into(),
                },
                "pass:placement:L1",
            ),
            (
                LeaseName::LifecyclePass {
                    library_id: "L1".into(),
                },
                "pass:lifecycle:L1",
            ),
            (
                LeaseName::Job {
                    job_id: "J1".into(),
                },
                "job:J1",
            ),
            (LeaseName::SetupClaim, "setup:claim"),
        ];
        for (name, expected) in cases {
            assert_eq!(name.as_text(), expected);
        }
    }

    #[test]
    fn placement_and_lifecycle_leases_over_one_library_do_not_collide() {
        let placement = LeaseName::PlacementPass {
            library_id: "L1".into(),
        };
        let lifecycle = LeaseName::LifecyclePass {
            library_id: "L1".into(),
        };
        assert_ne!(placement.as_text(), lifecycle.as_text());
    }

    #[test]
    fn a_task_owner_carries_the_instance_prefix() {
        let owner = LeaseOwner::task("INSTANCE", "collection-sync");
        assert!(
            owner
                .as_str()
                .starts_with(&LeaseOwner::instance_prefix("INSTANCE"))
        );
    }

    #[test]
    fn a_token_owner_does_not_carry_an_instance_prefix() {
        let owner = LeaseOwner::token("f00d");
        assert!(
            !owner
                .as_str()
                .starts_with(&LeaseOwner::instance_prefix("INSTANCE"))
        );
    }
}
