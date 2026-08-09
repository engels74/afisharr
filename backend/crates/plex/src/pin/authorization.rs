// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The middle step: the only part where PIN and OAuth differ.

use url::Url;

use crate::identity::ClientIdentity;

/// Where plex.tv's hosted sign-in lives.
const AUTH_APP_BASE: &str = "https://app.plex.tv/auth";

/// Which of the two login shapes a pin was created for.
///
/// The pin resource, the polling, the expiry handling, and the token's
/// destination are identical. Only what the operator is shown differs, so this
/// enum decides one thing and the rest of the flow never branches on it again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Show the four-character code; the operator types it at plex.tv/link.
    Pin,
    /// Send the operator to plex.tv's hosted sign-in and back again.
    OAuth,
}

impl Mode {
    /// The text stored in `plex_pin_logins.mode`.
    #[must_use]
    pub const fn as_text(self) -> &'static str {
        match self {
            Self::Pin => "Pin",
            Self::OAuth => "OAuth",
        }
    }

    /// Reads the value back from the column.
    #[must_use]
    pub fn from_text(text: &str) -> Option<Self> {
        match text {
            "Pin" => Some(Self::Pin),
            "OAuth" => Some(Self::OAuth),
            _ => None,
        }
    }
}

/// The hosted sign-in URL for an OAuth pin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationUrl(Url);

impl AuthorizationUrl {
    /// Builds the sign-in URL for `code`, returning the operator to
    /// `forward_to` afterwards.
    ///
    /// # Errors
    /// Returns the parse failure when the compiled-in base URL cannot be
    /// parsed, which is a build-time impossibility kept as a `Result` rather
    /// than an `expect` so no non-test path panics.
    pub fn build(
        identity: &ClientIdentity,
        code: &str,
        forward_to: &str,
    ) -> Result<Self, url::ParseError> {
        let mut url = Url::parse(AUTH_APP_BASE)?;
        // plex.tv reads these from the fragment, not the query string. A
        // fragment is also what keeps the code out of the operator's proxy
        // logs on the way to plex.tv.
        let fragment = format!(
            "?clientID={}&code={}&forwardUrl={}&context%5Bdevice%5D%5Bproduct%5D=Afisharr",
            encode(identity.client_identifier()),
            encode(code),
            encode(forward_to)
        );
        url.set_fragment(Some(&fragment));
        Ok(Self(url))
    }

    /// The URL the interface sends the operator to.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Percent-encodes everything that is not unreserved in RFC 3986.
///
/// Hand-rolled rather than pulled from a crate: the fragment is assembled by
/// hand because plex.tv wants bracketed keys that a form serialiser escapes
/// differently, and one encoder used in one place is easier to check than a
/// serialiser configured to produce a shape it was not designed for.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(byte));
            }
            other => {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                out.push('%');
                out.push(char::from(HEX[usize::from(other >> 4)]));
                out.push(char::from(HEX[usize::from(other & 0x0f)]));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> ClientIdentity {
        ClientIdentity::new("01JABCDEF", "Living Room", "0.1.0").expect("a valid identity")
    }

    #[test]
    fn every_mode_round_trips_through_its_column_text() {
        for mode in [Mode::Pin, Mode::OAuth] {
            assert_eq!(Mode::from_text(mode.as_text()), Some(mode));
        }
    }

    #[test]
    fn a_mode_the_schema_does_not_allow_does_not_parse() {
        assert_eq!(Mode::from_text("Token"), None);
    }

    #[test]
    fn the_authorization_url_carries_the_code_and_the_client_identifier() {
        let url = AuthorizationUrl::build(&identity(), "abcd", "https://afisharr.example/login")
            .expect("the compiled-in base parses");
        assert!(url.as_str().starts_with("https://app.plex.tv/auth#"));
        assert!(url.as_str().contains("code=abcd"), "{}", url.as_str());
        assert!(
            url.as_str().contains("clientID=01JABCDEF"),
            "{}",
            url.as_str()
        );
    }

    #[test]
    fn the_forward_url_is_percent_encoded_rather_than_pasted_in() {
        let url =
            AuthorizationUrl::build(&identity(), "abcd", "https://afisharr.example/login?x=1")
                .expect("the compiled-in base parses");
        assert!(
            url.as_str()
                .contains("forwardUrl=https%3A%2F%2Fafisharr.example%2Flogin%3Fx%3D1"),
            "{}",
            url.as_str()
        );
    }

    #[test]
    fn the_encoder_leaves_unreserved_characters_alone() {
        assert_eq!(encode("aZ0-_.~"), "aZ0-_.~");
        assert_eq!(encode("a b"), "a%20b");
    }
}
