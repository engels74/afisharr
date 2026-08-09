// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `X-Plex-*` header set every request carries.

use afisharr_sources::outbound::{HeaderName, HeaderValue};
use thiserror::Error;

/// The header plex.tv binds an issued token to. Never regenerated (PRD §19.5).
pub const PLEX_CLIENT_IDENTIFIER: HeaderName = HeaderName::from_static("x-plex-client-identifier");

/// The header a token is presented in.
pub const PLEX_TOKEN: HeaderName = HeaderName::from_static("x-plex-token");

const PLEX_PRODUCT: HeaderName = HeaderName::from_static("x-plex-product");
const PLEX_VERSION: HeaderName = HeaderName::from_static("x-plex-version");
const PLEX_DEVICE: HeaderName = HeaderName::from_static("x-plex-device");
const PLEX_DEVICE_NAME: HeaderName = HeaderName::from_static("x-plex-device-name");
const PLEX_PLATFORM: HeaderName = HeaderName::from_static("x-plex-platform");

/// The header plex.tv needs in order to answer JSON rather than XML.
const ACCEPT: HeaderName = HeaderName::from_static("accept");

/// Why an identity could not be built.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IdentityError {
    /// A field held a byte that cannot appear in an HTTP header value.
    #[error("{field} holds a character that cannot go in an HTTP header")]
    NotHeaderSafe {
        /// The field that was rejected.
        field: &'static str,
    },
}

/// Who Afisharr says it is, on every request to Plex.
///
/// Built once at startup from the `instance` row and cloned per request. The
/// values are pre-validated `HeaderValue`s so a device name holding a newline
/// fails at construction, where the operator typed it, and not on the request
/// that would have carried it.
#[derive(Debug, Clone)]
pub struct ClientIdentity {
    client_identifier: HeaderValue,
    product: HeaderValue,
    version: HeaderValue,
    device: HeaderValue,
    device_name: HeaderValue,
    platform: HeaderValue,
}

impl ClientIdentity {
    /// Builds the identity from the instance's own values.
    ///
    /// # Errors
    /// Returns [`IdentityError::NotHeaderSafe`] naming the field when a value
    /// cannot be sent as a header.
    pub fn new(
        client_identifier: &str,
        device_name: &str,
        app_version: &str,
    ) -> Result<Self, IdentityError> {
        Ok(Self {
            client_identifier: value(client_identifier, "clientIdentifier")?,
            product: HeaderValue::from_static("Afisharr"),
            version: value(app_version, "appVersion")?,
            device: HeaderValue::from_static("Afisharr"),
            device_name: value(device_name, "deviceName")?,
            platform: value(std::env::consts::OS, "platform")?,
        })
    }

    /// The client identifier plex.tv binds tokens to.
    #[must_use]
    pub fn client_identifier(&self) -> &str {
        self.client_identifier.to_str().unwrap_or_default()
    }

    /// The header set, ready to hand to the outbound client.
    ///
    /// `Accept: application/json` is part of it rather than a per-call
    /// addition: plex.tv answers XML without it, and one call site forgetting
    /// is a parse failure nothing else explains.
    #[must_use]
    pub fn headers(&self) -> Vec<(HeaderName, HeaderValue)> {
        vec![
            (ACCEPT, HeaderValue::from_static("application/json")),
            (PLEX_CLIENT_IDENTIFIER, self.client_identifier.clone()),
            (PLEX_PRODUCT, self.product.clone()),
            (PLEX_VERSION, self.version.clone()),
            (PLEX_DEVICE, self.device.clone()),
            (PLEX_DEVICE_NAME, self.device_name.clone()),
            (PLEX_PLATFORM, self.platform.clone()),
        ]
    }
}

fn value(text: &str, field: &'static str) -> Result<HeaderValue, IdentityError> {
    HeaderValue::from_str(text).map_err(|_| IdentityError::NotHeaderSafe { field })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> ClientIdentity {
        ClientIdentity::new("01JABCDEF", "Living Room", "0.1.0").expect("a valid identity")
    }

    #[test]
    fn the_header_set_carries_the_client_identifier_and_asks_for_json() {
        let headers = identity().headers();
        let named = |name: &HeaderName| {
            headers
                .iter()
                .find(|(header, _)| header == name)
                .map(|(_, value)| value.to_str().unwrap_or_default().to_owned())
        };
        assert_eq!(named(&PLEX_CLIENT_IDENTIFIER).as_deref(), Some("01JABCDEF"));
        assert_eq!(named(&ACCEPT).as_deref(), Some("application/json"));
        assert_eq!(named(&PLEX_DEVICE_NAME).as_deref(), Some("Living Room"));
    }

    #[test]
    fn the_token_header_is_not_part_of_the_identity() {
        // A token is per-request and per-account; baking it into the identity
        // would make every call carry whichever one was set up first.
        assert!(
            identity()
                .headers()
                .iter()
                .all(|(name, _)| *name != PLEX_TOKEN)
        );
    }

    #[test]
    fn a_device_name_that_cannot_be_a_header_is_refused_naming_the_field() {
        let error = ClientIdentity::new("01JABCDEF", "Living\nRoom", "0.1.0")
            .expect_err("a newline must be refused");
        assert!(error.to_string().contains("deviceName"), "{error}");
    }

    #[test]
    fn the_client_identifier_reads_back_as_written() {
        assert_eq!(identity().client_identifier(), "01JABCDEF");
    }
}
