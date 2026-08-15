// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The server token, as a value that does not print itself.

use afisharr_sources::outbound::HeaderValue;

use crate::identity::IdentityError;

/// The `X-Plex-Token` this client presents to the server.
///
/// A newtype over a pre-validated `HeaderValue` for two reasons. A token
/// holding a newline fails where the operator supplied it rather than on the
/// request that would have carried it; and the `Debug` implementation below
/// prints nothing, so a `#[instrument]` attribute that captures the client
/// cannot put the operator's credential in `logs/afisharr.log` (D-032).
#[derive(Clone)]
pub struct ServerToken(HeaderValue);

impl ServerToken {
    /// Builds a token from what the `secrets` table decrypted.
    ///
    /// # Errors
    /// Returns [`IdentityError::NotHeaderSafe`] when the value cannot be sent
    /// as a header.
    pub fn new(value: &str) -> Result<Self, IdentityError> {
        let mut header =
            HeaderValue::from_str(value).map_err(|_| IdentityError::NotHeaderSafe {
                field: "the Plex server token",
            })?;
        // Marked sensitive so anything downstream that redacts headers by this
        // flag — `HeaderMap`'s own `Debug`, and the tracing integrations built
        // on it — redacts this one without being told which name it has.
        header.set_sensitive(true);
        Ok(Self(header))
    }

    /// The header value, for the one place that builds the header set.
    pub(crate) fn header_value(&self) -> HeaderValue {
        self.0.clone()
    }
}

impl std::fmt::Debug for ServerToken {
    /// Prints that a token is present, and never which one.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ServerToken(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_never_prints_itself() {
        let token = ServerToken::new("xyzzy-plex-token").expect("a header-safe token");
        let printed = format!("{token:?}");
        assert!(!printed.contains("xyzzy"), "{printed}");
    }

    #[test]
    fn a_token_that_cannot_be_a_header_is_refused_where_it_is_supplied() {
        let error = ServerToken::new("tok\nen").expect_err("a newline is not header-safe");
        assert!(error.to_string().contains("Plex server token"), "{error}");
    }

    #[test]
    fn the_header_value_is_marked_sensitive() {
        let token = ServerToken::new("xyzzy").expect("a header-safe token");
        assert!(token.header_value().is_sensitive());
    }
}
