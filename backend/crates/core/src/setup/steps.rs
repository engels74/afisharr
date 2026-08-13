// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The eight wizard steps, and the one function that decides which is current.

use serde::Serialize;

use crate::setup::Evidence;

/// One step of the setup journey (PRD §7.14).
///
/// Ordered, and the order is the resume rule: the current step is the first one
/// whose evidence is absent. A client cannot name a step — that is what D-046
/// forbids, and the absence of any `from_index` here is how the type enforces
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SetupStep {
    /// Prove console access and take the wizard.
    Claim,
    /// Create the administrator account.
    Admin,
    /// Connect the Plex server.
    Plex,
    /// Choose which libraries Afisharr manages.
    Libraries,
    /// Configure the integrations, of which TMDB is required.
    Integrations,
    /// Choose starter packs, or choose none.
    Packs,
    /// Read the report on collections that already exist.
    Report,
    /// Review and finish.
    Review,
}

impl SetupStep {
    /// The step's position in the journey, one-based, for display only.
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Claim => 1,
            Self::Admin => 2,
            Self::Plex => 3,
            Self::Libraries => 4,
            Self::Integrations => 5,
            Self::Packs => 6,
            Self::Report => 7,
            Self::Review => 8,
        }
    }

    /// The step this instance resumes at, derived from `evidence`.
    ///
    /// Every arm reads a fact the wizard writes, never a fact the wizard
    /// remembers writing. A resumed wizard on an instance where the write
    /// failed reports the step again rather than skipping it (`I-UX-10`).
    #[must_use]
    pub const fn resume_at(evidence: Evidence) -> Self {
        if !evidence.claim_held_by_caller {
            Self::Claim
        } else if !evidence.admin_exists {
            Self::Admin
        } else if !evidence.plex_connected {
            Self::Plex
        } else if !evidence.library_selected {
            Self::Libraries
        } else if !evidence.tmdb_configured {
            Self::Integrations
        } else if !evidence.packs_acknowledged {
            Self::Packs
        } else if !evidence.report_acknowledged {
            Self::Report
        } else {
            Self::Review
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Evidence with every fact present, which resumes at Review.
    const fn complete() -> Evidence {
        Evidence {
            claim_held_by_caller: true,
            admin_exists: true,
            plex_connected: true,
            library_selected: true,
            tmdb_configured: true,
            packs_acknowledged: true,
            report_acknowledged: true,
        }
    }

    #[test]
    fn an_empty_instance_resumes_at_the_claim() {
        let evidence = Evidence::default();
        assert_eq!(SetupStep::resume_at(evidence), SetupStep::Claim);
    }

    #[test]
    fn every_step_is_reached_by_removing_exactly_its_own_evidence() {
        /// One case: a mutation that removes a step's evidence, and the step
        /// the resume rule must then report.
        type Case = (fn(&mut Evidence), SetupStep);

        let cases: [Case; 8] = [
            (|e| e.claim_held_by_caller = false, SetupStep::Claim),
            (|e| e.admin_exists = false, SetupStep::Admin),
            (|e| e.plex_connected = false, SetupStep::Plex),
            (|e| e.library_selected = false, SetupStep::Libraries),
            (|e| e.tmdb_configured = false, SetupStep::Integrations),
            (|e| e.packs_acknowledged = false, SetupStep::Packs),
            (|e| e.report_acknowledged = false, SetupStep::Report),
            (|_| (), SetupStep::Review),
        ];
        for (remove, expected) in cases {
            let mut evidence = complete();
            remove(&mut evidence);
            assert_eq!(SetupStep::resume_at(evidence), expected);
        }
    }

    #[test]
    fn a_later_step_being_satisfied_does_not_skip_an_earlier_gap() {
        // The failure this prevents: an instance whose Plex write failed but
        // whose packs were acknowledged reporting step 6.
        let mut evidence = complete();
        evidence.plex_connected = false;
        assert_eq!(SetupStep::resume_at(evidence), SetupStep::Plex);
    }

    #[test]
    fn the_ordinals_run_one_through_eight_without_a_gap() {
        let steps = [
            SetupStep::Claim,
            SetupStep::Admin,
            SetupStep::Plex,
            SetupStep::Libraries,
            SetupStep::Integrations,
            SetupStep::Packs,
            SetupStep::Report,
            SetupStep::Review,
        ];
        let ordinals: Vec<u8> = steps.iter().map(|step| step.ordinal()).collect();
        assert_eq!(ordinals, (1..=8).collect::<Vec<u8>>());
    }

    #[test]
    fn the_step_serialises_as_a_name_rather_than_a_number() {
        let encoded = serde_json::to_string(&SetupStep::Integrations).expect("serialises");
        assert_eq!(encoded, "\"integrations\"");
    }
}
