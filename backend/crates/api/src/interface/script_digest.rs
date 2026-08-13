// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Hashing the shell's inline scripts, so the policy can admit exactly those.

use afisharr_core::digest;

/// The `sha256-…` source expressions for every inline script in `html`.
///
/// `adapter-static` emits one inline module that starts the client router;
/// there is no supported way to make `SvelteKit` omit it, and `'unsafe-inline'`
/// would admit any script an injection managed to place. Hashing the bytes the
/// binary is about to serve admits that one script and nothing else, and the
/// digest tracks the bundle automatically because it is computed from it.
///
/// A `<script src=…>` is not inline and is covered by `'self'`, so it is
/// skipped.
#[must_use]
pub fn inline_script_digests(html: &str) -> Vec<String> {
    let mut digests = Vec::new();
    let mut rest = html;

    while let Some(open) = rest.find("<script") {
        rest = &rest[open..];
        let Some(tag_end) = rest.find('>') else { break };
        let tag = &rest[..tag_end];
        let Some(close) = rest[tag_end..].find("</script") else {
            break;
        };
        let body = &rest[tag_end + 1..tag_end + close];

        if !tag.contains("src=") {
            digests.push(digest::csp_source(body));
        }
        rest = &rest[tag_end + close..];
    }

    digests.sort_unstable();
    digests.dedup();
    digests
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_inline_script_is_hashed() {
        let digests = inline_script_digests("<html><body><script>start()</script></body></html>");
        assert_eq!(digests, vec![digest::csp_source("start()")]);
    }

    #[test]
    fn an_external_script_is_not_hashed() {
        let digests = inline_script_digests(r#"<script src="/app.js"></script>"#);
        assert!(digests.is_empty(), "{digests:?}");
    }

    #[test]
    fn a_script_with_attributes_is_still_hashed_by_its_body() {
        let digests = inline_script_digests(r#"<script type="module">start()</script>"#);
        assert_eq!(digests, vec![digest::csp_source("start()")]);
    }

    #[test]
    fn two_identical_scripts_yield_one_digest() {
        let digests = inline_script_digests("<script>a()</script><script>a()</script>");
        assert_eq!(digests.len(), 1);
    }

    #[test]
    fn several_scripts_are_all_admitted() {
        let digests = inline_script_digests("<script>a()</script><script>b()</script>");
        assert_eq!(digests.len(), 2);
        assert!(digests.contains(&digest::csp_source("a()")));
        assert!(digests.contains(&digest::csp_source("b()")));
    }

    #[test]
    fn html_with_no_script_yields_nothing() {
        assert!(inline_script_digests("<html><body>hi</body></html>").is_empty());
    }

    #[test]
    fn an_unterminated_script_tag_does_not_loop_forever() {
        assert!(inline_script_digests("<script>start()").is_empty());
        assert!(inline_script_digests("<script").is_empty());
    }

    #[test]
    fn the_digest_of_the_real_shell_shape_is_stable() {
        // The shape adapter-static actually emits: a preload link, then one
        // inline module. Pinned so a change to the extractor shows up here
        // rather than as a blank page behind a CSP nobody suspects.
        let shell = concat!(
            "<link href=\"/_app/immutable/entry/start.js\" rel=\"modulepreload\">",
            "<script>\n\t{ kit.start(app, element); }\n</script>"
        );
        let digests = inline_script_digests(shell);
        assert_eq!(
            digests,
            vec![digest::csp_source("\n\t{ kit.start(app, element); }\n")]
        );
    }
}
