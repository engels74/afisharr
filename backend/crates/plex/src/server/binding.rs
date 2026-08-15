// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `I-ID-5` — a different server is a different world.

use crate::server::MachineIdentifier;

/// What comparing the observed identifier against the recorded one decided.
///
/// Three outcomes and not a boolean, because "no server has ever been bound"
/// and "the bound server answered with a different identifier" call for
/// opposite behaviour: the first is a first run, and the second suspends
/// everything until an operator decides (P1, PRD §19.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingVerdict {
    /// Nothing has been bound yet. Whatever answered is a candidate.
    Unbound {
        /// What answered.
        found: MachineIdentifier,
    },
    /// The server that answered is the server that was bound.
    Bound {
        /// The identifier both sides agree on.
        identifier: MachineIdentifier,
    },
    /// A different server answered at the configured address.
    ///
    /// Every rating key, adoption, discovered field, and placement position in
    /// the database means something else on this server. Nothing is rebound
    /// across it, and nothing is written: the operator decides between "this is
    /// a new server, rebind" and "restore a backup" (`I-ID-5`).
    DifferentServer {
        /// The identifier the database is bound to.
        expected: MachineIdentifier,
        /// The identifier that answered.
        found: MachineIdentifier,
    },
}

impl BindingVerdict {
    /// Whether this verdict blocks every Plex-bound action.
    #[must_use]
    pub const fn blocks(&self) -> bool {
        matches!(self, Self::DifferentServer { .. })
    }
}

/// Compares what answered against what the database is bound to.
///
/// A pure function over two values, deliberately: it takes no clock, no
/// database, and no client, so the rule `I-ID-5` states is one exhaustive match
/// that a table-driven test covers completely, and there is exactly one
/// implementation of it in the product (P7).
#[must_use]
pub fn verify_binding(
    recorded: Option<&MachineIdentifier>,
    observed: &MachineIdentifier,
) -> BindingVerdict {
    match recorded {
        None => BindingVerdict::Unbound {
            found: observed.clone(),
        },
        Some(recorded) if recorded == observed => BindingVerdict::Bound {
            identifier: observed.clone(),
        },
        Some(recorded) => BindingVerdict::DifferentServer {
            expected: recorded.clone(),
            found: observed.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identifier(value: &str) -> MachineIdentifier {
        MachineIdentifier::new(value)
    }

    #[test]
    fn a_first_run_binds_nothing_and_blocks_nothing() {
        let verdict = verify_binding(None, &identifier("abc"));
        assert_eq!(
            verdict,
            BindingVerdict::Unbound {
                found: identifier("abc")
            }
        );
        assert!(!verdict.blocks());
    }

    #[test]
    fn the_same_server_answering_again_is_bound() {
        let verdict = verify_binding(Some(&identifier("abc")), &identifier("abc"));
        assert!(!verdict.blocks());
        assert!(matches!(verdict, BindingVerdict::Bound { .. }));
    }

    #[test]
    fn a_different_identifier_blocks_and_names_both_sides() {
        // Naming both is what makes the operator's decision possible: an answer
        // that said only "wrong server" leaves them unable to tell a swapped
        // container from a restored backup.
        let verdict = verify_binding(Some(&identifier("abc")), &identifier("xyz"));
        assert!(verdict.blocks());
        assert_eq!(
            verdict,
            BindingVerdict::DifferentServer {
                expected: identifier("abc"),
                found: identifier("xyz"),
            }
        );
    }

    #[test]
    fn the_comparison_is_exact_rather_than_case_folded_or_trimmed() {
        // Plex machine identifiers are opaque. Normalising one is inventing a
        // rule about somebody else's identifier space, and a rule that says two
        // different servers are the same is the write `I-ID-5` exists to stop.
        assert!(verify_binding(Some(&identifier("ABC")), &identifier("abc")).blocks());
        assert!(verify_binding(Some(&identifier("abc ")), &identifier("abc")).blocks());
    }
}
