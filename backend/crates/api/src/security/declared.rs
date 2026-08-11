// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Which origin a request says it came from, and whether that is this instance.
//!
//! Split out of the CSRF judge because it answers a different question. The
//! judge decides what to do about a foreign origin; this decides what counts as
//! one, and that turns entirely on which addresses this instance is genuinely
//! served at — a question about the deployment rather than about forgery.

use axum::http::{
    HeaderMap,
    header::{HOST, ORIGIN, REFERER},
};

use crate::proxy::PublicOrigin;

/// The URL the request says it came from, as it was written.
///
/// `Origin` first, `Referer` as the fallback: `Origin` is the one designed for
/// this and is sent on every cross-origin state-changing request, and `Referer`
/// is what remains when a privacy setting has stripped it down.
///
/// Kept whole rather than reduced to a host, because the configured origin is
/// compared as an origin — scheme, host, and port — and a value with its scheme
/// already thrown away cannot be. An opaque origin arrives as the literal
/// `null`, which parses as no origin at all and so matches nothing; returning
/// it rather than `None` is what keeps a sandboxed frame from skipping the
/// check entirely.
pub(super) fn declared_origin(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get(ORIGIN)
        .or_else(|| headers.get(REFERER))?
        .to_str()
        .ok()?;
    Some(raw.to_owned())
}

/// Whether the declared origin is this instance.
///
/// Either address is accepted: the configured `publicOrigin`, and the request's
/// own `Host`. Both are needed, and neither lets a caller nominate an origin of
/// their own.
///
/// `Host` alone is what breaks behind a proxy that rewrites it — nginx's
/// `proxy_pass` with no `proxy_set_header Host` sets it to the upstream's own
/// name, so the instance sees `Host: afisharr:8484` beside
/// `Origin: https://media.example` and refuses every write the operator makes.
/// `publicOrigin` alone is what breaks the operator who also reaches the
/// instance directly on the LAN, at an address the configured origin does not
/// cover — and that address is one this instance is genuinely served at.
///
/// Accepting `Host` is not a hole in the check: a browser writes `Host` from
/// the URL it is posting to, so a page on another site cannot make one say
/// anything but the instance's own address, which is exactly the forgery being
/// judged. What a caller *can* choose is `Host` and `Origin` together, and
/// matching them to each other still tells this instance the request did not
/// come from another site.
pub(super) fn declares_this_instance(
    declared: &str,
    headers: &HeaderMap,
    configured: Option<&PublicOrigin>,
) -> bool {
    if configured.is_some_and(|origin| origin.covers(declared)) {
        return true;
    }

    let Some(declared_host) = host_of(declared) else {
        return false;
    };
    let host = headers
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    !declared_host.is_empty() && !host.is_empty() && declared_host.eq_ignore_ascii_case(host)
}

/// The host-and-port part of a declared origin, for the `Host` comparison.
fn host_of(declared: &str) -> Option<&str> {
    let without_scheme = declared.split_once("://").map(|(_, rest)| rest)?;
    Some(
        without_scheme
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderName, HeaderValue};

    use super::*;

    fn headers(entries: &[(HeaderName, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in entries {
            map.insert(name, HeaderValue::from_str(value).expect("a valid header"));
        }
        map
    }

    fn configured(value: &str) -> PublicOrigin {
        PublicOrigin::parse(value).expect("a valid origin")
    }

    #[test]
    fn the_origin_header_is_read_first_and_the_referer_stands_in_for_it() {
        let both = headers(&[
            (ORIGIN, "https://media.example"),
            (REFERER, "https://evil.example/page"),
        ]);
        assert_eq!(
            declared_origin(&both).as_deref(),
            Some("https://media.example")
        );

        let stripped = headers(&[(REFERER, "https://evil.example/page")]);
        assert_eq!(
            declared_origin(&stripped).as_deref(),
            Some("https://evil.example/page")
        );

        assert_eq!(declared_origin(&HeaderMap::new()), None);
    }

    #[test]
    fn a_request_at_the_host_it_declares_is_this_instance() {
        let map = headers(&[(HOST, "afisharr.example")]);
        assert!(declares_this_instance(
            "https://afisharr.example",
            &map,
            None
        ));
        assert!(declares_this_instance(
            "https://AFISHARR.EXAMPLE/page",
            &map,
            None
        ));
    }

    #[test]
    fn a_proxy_that_rewrites_host_does_not_make_every_write_foreign() {
        // The deployment this closes, and it is the ordinary one: nginx's
        // `proxy_pass` sets `Host` to the upstream's own name. Judged against
        // `Host` alone, every write an operator makes is a cross-site attack —
        // sign-in, the setup claim, and creating the administrator included.
        let map = headers(&[(HOST, "afisharr:8484")]);
        assert!(!declares_this_instance("https://media.example", &map, None));
        assert!(declares_this_instance(
            "https://media.example",
            &map,
            Some(&configured("https://media.example"))
        ));
    }

    #[test]
    fn configuring_an_origin_does_not_lock_out_the_address_on_the_lan() {
        // An operator who set `publicOrigin` to their public address still
        // reaches the same instance at `http://192.168.1.10:8484`. Judging
        // against the configured origin alone would refuse every write they
        // make there, which is the reported failure moved rather than fixed.
        let map = headers(&[(HOST, "192.168.1.10:8484")]);
        assert!(declares_this_instance(
            "http://192.168.1.10:8484",
            &map,
            Some(&configured("https://media.example"))
        ));
    }

    #[test]
    fn an_origin_that_is_neither_address_is_foreign() {
        let origin = configured("https://media.example");
        let map = headers(&[(HOST, "media.example")]);
        for declared in [
            "https://evil.example",
            "https://media.example.evil.example",
            "https://media.example@evil.example",
            "null",
            "",
            "not a url",
        ] {
            assert!(
                !declares_this_instance(declared, &map, Some(&origin)),
                "'{declared}' must not be read as this instance"
            );
        }
    }

    #[test]
    fn a_request_with_no_host_header_declares_nothing_this_instance_can_confirm() {
        assert!(!declares_this_instance(
            "https://afisharr.example",
            &HeaderMap::new(),
            None
        ));
    }
}
