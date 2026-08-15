// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Taking a password out of text on its way to a screen.
//!
//! Its own module rather than a corner of the address type, because the two are
//! used at different moments: an address is redacted once, when it is parsed,
//! and text is redacted every time a message that quotes one is rendered. The
//! second is the harder half — it has to cope with text `Url` will not parse at
//! all — and it is the half a reviewer needs to read on its own.

use url::Url;

/// What a redacted password is rendered as.
///
/// Not the empty string: an address that showed `http://admin@plex.lan` would
/// read as one configured without a password at all, and the operator checking
/// why their proxy refuses the request needs to see that one is being sent.
const REDACTED: &str = "***";

/// `text` with any password in it replaced by [`REDACTED`].
///
/// A base address is whatever the operator configured, and an operator whose
/// server sits behind a reverse proxy configures `http://user:secret@plex.lan`
/// — a secret this build then holds in a string that is displayed, logged, and
/// returned to the browser. The password is kept for the request and removed
/// from every rendering of it.
///
/// Takes text rather than an address, so that a base which never parsed is
/// covered too: the failure messages in `AddressError` quote what the operator
/// typed, and that is the one rendering most likely to be pasted into a bug
/// report.
#[must_use]
pub fn redact_credentials(text: &str) -> String {
    match Url::parse(text.trim()) {
        Ok(mut url) if url.password().is_some() => {
            if url.set_password(Some(REDACTED)).is_ok() {
                return url.into();
            }
            scrub(text)
        }
        // Nothing to hide, and the text is returned exactly as it arrived: a
        // round trip through `Url` would normalise an address the operator has
        // to recognise.
        Ok(_) => text.to_owned(),
        // Not a URL, which does not mean not a credential: `http://u:p@ plex`
        // fails to parse and still names a password.
        Err(_) => scrub(text),
    }
}

/// The same redaction, by hand, for text `Url` will not parse.
fn scrub(text: &str) -> String {
    let Some((before, rest)) = text.split_once("://") else {
        return text.to_owned();
    };
    // The authority ends where the path, query, or fragment begins; a `@` past
    // that point belongs to some other part of the text.
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let Some((userinfo, host)) = authority.rsplit_once('@') else {
        return text.to_owned();
    };
    match userinfo.split_once(':') {
        Some((user, _)) => format!("{before}://{user}:{REDACTED}@{host}{tail}"),
        // A username and no password is not a secret.
        None => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::ServerAddress;

    #[test]
    fn redaction_covers_the_text_of_an_address_that_never_parsed() {
        // `AddressError` quotes what the operator typed, and that message is
        // the rendering most likely to be pasted into a bug report.
        let error = ServerAddress::parse("http://admin:hunter2@ plex.lan")
            .expect_err("a space is not a host");
        let detail = redact_credentials(&error.to_string());
        assert!(!detail.contains("hunter2"), "{detail}");
        assert!(detail.contains("admin:***@"), "{detail}");
    }

    #[test]
    fn redaction_leaves_an_address_with_nothing_to_hide_exactly_as_it_arrived() {
        // Byte for byte: an operator checking the address on the page has to
        // recognise what they typed, and a round trip through `Url` would
        // normalise it under them.
        for text in [
            "http://plex.lan:32400",
            "https://home.example/pms/",
            "http://admin@plex.lan",
            "not an address at all",
        ] {
            assert_eq!(redact_credentials(text), text);
        }
    }

    #[test]
    fn a_password_is_hidden_wherever_in_the_text_it_sits() {
        assert_eq!(
            redact_credentials("http://admin:hunter2@plex.lan:32400/pms?a=1#b"),
            "http://admin:***@plex.lan:32400/pms?a=1#b"
        );
        // And an `@` past the authority belongs to the path, not to a userinfo.
        assert_eq!(
            redact_credentials("http://plex.lan/a@b"),
            "http://plex.lan/a@b"
        );
    }
}
