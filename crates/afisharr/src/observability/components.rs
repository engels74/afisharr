// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The component crates linked into this build.

/// One component crate present in the running binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Component {
    /// The crate name.
    pub name: &'static str,
    /// The version compiled in.
    pub version: &'static str,
}

/// Every component crate this binary was linked against.
///
/// Read from each crate's own compiled-in constant rather than from a list
/// maintained here, so a component that is dropped from the binary disappears
/// from the boot log instead of continuing to be reported (P1: the log states
/// what is present, never what was expected).
#[must_use]
pub fn components() -> [Component; 6] {
    [
        Component {
            name: "afisharr-core",
            version: afisharr_core::VERSION,
        },
        Component {
            name: "afisharr-api",
            version: afisharr_api::VERSION,
        },
        Component {
            name: "afisharr-plex",
            version: afisharr_plex::VERSION,
        },
        Component {
            name: "afisharr-sources",
            version: afisharr_sources::VERSION,
        },
        Component {
            name: "afisharr-render",
            version: afisharr_render::VERSION,
        },
        Component {
            name: "afisharr-packs",
            version: afisharr_packs::VERSION,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_component_reports_a_name_and_a_version() {
        for component in components() {
            assert!(!component.name.is_empty());
            assert!(
                !component.version.is_empty(),
                "{} reported no version",
                component.name
            );
        }
    }

    #[test]
    fn no_component_is_listed_twice() {
        let mut names: Vec<&str> = components().iter().map(|c| c.name).collect();
        names.sort_unstable();
        let listed = names.len();
        names.dedup();
        assert_eq!(names.len(), listed);
    }
}
