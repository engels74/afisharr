// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The handful of instance facts the HTTP surface reads on every request.

use afisharr_core::locale::LocaleTag;

/// Who this instance is, from the HTTP surface's point of view.
///
/// A snapshot taken at boot rather than a read of the `instance` row per
/// request: `client_identifier` is immutable by construction (PRD §19.5), and
/// the rest change only through a restart or a settings save that rebuilds the
/// router's state.
#[derive(Debug, Clone)]
pub struct InstanceIdentity {
    /// ULID of this installation.
    pub instance_id: String,
    /// `X-Plex-Client-Identifier`.
    pub client_identifier: String,
    /// The interface language this instance formats in.
    pub locale: LocaleTag,
    /// The version of the running binary, reported by the health route.
    pub app_version: String,
    /// Whether `instance.setup_completed_at` was set at boot.
    ///
    /// Read from the row at boot and flipped in memory when setup completes,
    /// so the gate does not run a query on every request to answer a question
    /// that changes once in the life of an instance.
    pub setup_completed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identity_carries_the_client_identifier_verbatim() {
        let identity = InstanceIdentity {
            instance_id: "01JINSTANCE".to_owned(),
            client_identifier: "01JCLIENT".to_owned(),
            locale: LocaleTag::default(),
            app_version: "0.1.0".to_owned(),
            setup_completed: false,
        };
        assert_eq!(identity.client_identifier, "01JCLIENT");
        assert_eq!(identity.locale.as_str(), "en");
    }
}
