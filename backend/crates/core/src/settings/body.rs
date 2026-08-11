// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The typed settings document.

use serde::{Deserialize, Serialize};

/// The whole configuration document, as stored in `settings.body_json`.
///
/// Every group rejects unknown fields, so a typo in a mounted config file is an
/// error naming the field rather than a setting that silently does nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct SettingsBody {
    /// Identity and localisation of this installation.
    pub instance: InstanceSettings,
    /// The HTTP surface and what it trusts in front of it.
    pub http: HttpSettings,
    /// Bounds on the render cache.
    pub render: RenderSettings,
    /// The application log.
    pub logging: LoggingSettings,
    /// Retention of automatic backups.
    pub backup: BackupSettings,
}

/// Identity and localisation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct InstanceSettings {
    /// The name this instance presents to Plex.
    pub device_name: String,
    /// IANA timezone. The engine's date operators are day-aligned in it, and
    /// the lifecycle model computes phase from a civil-date difference, so a
    /// change here changes what existing definitions mean.
    pub timezone: String,
    /// Interface language tag.
    pub locale: String,
}

impl Default for InstanceSettings {
    fn default() -> Self {
        Self {
            device_name: "Afisharr".to_owned(),
            timezone: "UTC".to_owned(),
            locale: "en".to_owned(),
        }
    }
}

/// The HTTP surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct HttpSettings {
    /// Address to bind.
    pub bind_address: String,
    /// Port to bind.
    pub port: u16,
    /// Proxy addresses or CIDR ranges whose forwarded headers are honoured.
    ///
    /// A list, never a boolean (D-029, PRD §21.4.3). An empty list means
    /// forwarded headers are ignored and the peer address is used — a
    /// forged `X-Forwarded-For` then buys nothing.
    pub trust_proxy: Vec<String>,
    /// The absolute origin operators reach this instance at, if one is set.
    ///
    /// `https://afisharr.example` or `http://192.168.1.10:8484` — scheme, host,
    /// and port; anything after them is ignored. Nothing derives it from the
    /// request, because the request's `Host` belongs to whoever sent it: an
    /// instance that trusted that header would mint a plex.tv sign-in returning
    /// the operator to whatever address the caller asked for.
    ///
    /// Unset by default, and what it gates is the hosted plex.tv sign-in, which
    /// is the one flow that hands an absolute URL for this instance to somebody
    /// else. The code sign-in needs no return target and works without it.
    pub public_origin: Option<String>,
}

impl Default for HttpSettings {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_owned(),
            port: 8484,
            trust_proxy: Vec::new(),
            public_origin: None,
        }
    }
}

/// Bounds on the render cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct RenderSettings {
    /// Cap on the render cache in bytes, evicted least-recently-used.
    ///
    /// An unbounded cache at this scale is a disk-exhaustion bug with a delay
    /// fuse (`I-PERF-2`), so the cap has a default rather than being optional.
    pub cache_cap_bytes: u64,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            cache_cap_bytes: 10 * 1024 * 1024 * 1024,
        }
    }
}

/// The application log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct LoggingSettings {
    /// Tracing filter directive, e.g. `info` or `afisharr_core=debug,info`.
    pub level: String,
    /// How many log files to keep, counting the one being written.
    pub retained_files: u16,
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            level: "info".to_owned(),
            retained_files: 7,
        }
    }
}

/// Retention of automatic backups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct BackupSettings {
    /// How many pre-migration copies to retain (PRD §19.3 fixes this at three).
    pub retained_pre_migration: u16,
}

impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            retained_pre_migration: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn an_unknown_field_is_rejected_rather_than_ignored() {
        let error = serde_json::from_str::<SettingsBody>(r#"{"htpp": {}}"#)
            .expect_err("a typo must not deserialise");
        assert!(
            error.to_string().contains("htpp"),
            "the message must name the field: {error}"
        );
    }

    #[test]
    fn an_unknown_field_inside_a_group_is_rejected_too() {
        let error = serde_json::from_str::<SettingsBody>(r#"{"http": {"trustProxi": []}}"#)
            .expect_err("a typo inside a group must not deserialise");
        assert!(error.to_string().contains("trustProxi"), "{error}");
    }

    #[test]
    fn an_empty_body_deserialises_to_the_documented_defaults() {
        let body: SettingsBody = serde_json::from_str("{}").unwrap();
        assert_eq!(body, SettingsBody::default());
        assert!(
            body.http.trust_proxy.is_empty(),
            "no proxy is trusted by default"
        );
        assert!(
            body.http.public_origin.is_none(),
            "nothing is assumed about the address this instance is reached at"
        );
        assert_eq!(body.render.cache_cap_bytes, 10 * 1024 * 1024 * 1024);
        assert_eq!(body.backup.retained_pre_migration, 3);
    }

    #[test]
    fn the_body_round_trips_through_json() {
        let mut body = SettingsBody::default();
        body.http.trust_proxy.push("10.0.0.0/8".to_owned());
        body.http.public_origin = Some("https://afisharr.example".to_owned());
        let encoded = serde_json::to_string(&body).unwrap();
        assert_eq!(
            serde_json::from_str::<SettingsBody>(&encoded).unwrap(),
            body
        );
    }

    #[test]
    fn field_names_are_camel_case_on_the_wire() {
        let encoded = serde_json::to_string(&SettingsBody::default()).unwrap();
        assert!(encoded.contains("\"trustProxy\""), "{encoded}");
        assert!(encoded.contains("\"publicOrigin\""), "{encoded}");
        assert!(encoded.contains("\"cacheCapBytes\""), "{encoded}");
    }
}
