// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The wire form of the wizard's step.

use afisharr_core::setup::SetupStep;
use serde::Serialize;
use utoipa::ToSchema;

/// A step, as the generated client sees it.
///
/// A mirror of [`SetupStep`] rather than a `ToSchema` derive on the domain
/// type, so `afisharr-core` stays free of the HTTP surface's annotations
/// (§24.6.1). The `From` below is exhaustive, so a step added to the domain
/// fails to compile here rather than reaching the client as a string it cannot
/// narrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum StepView {
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

impl From<SetupStep> for StepView {
    fn from(step: SetupStep) -> Self {
        match step {
            SetupStep::Claim => Self::Claim,
            SetupStep::Admin => Self::Admin,
            SetupStep::Plex => Self::Plex,
            SetupStep::Libraries => Self::Libraries,
            SetupStep::Integrations => Self::Integrations,
            SetupStep::Packs => Self::Packs,
            SetupStep::Report => Self::Report,
            SetupStep::Review => Self::Review,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_domain_step_has_a_wire_form_that_serialises_the_same_way() {
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
        for step in steps {
            assert_eq!(
                serde_json::to_string(&StepView::from(step)).expect("serialises"),
                serde_json::to_string(&step).expect("serialises"),
                "{step:?} renders differently on the wire than in the domain"
            );
        }
    }
}
