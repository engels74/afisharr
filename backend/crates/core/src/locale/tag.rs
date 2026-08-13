// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A validated BCP 47 language tag.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The tag every instance ships a complete catalogue for.
pub const DEFAULT: &str = "en";

/// Why a string is not a language tag Afisharr will carry.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum LocaleTagError {
    /// The tag was empty, over-long, or held something other than ASCII
    /// letters, digits, and hyphens arranged as subtags.
    #[error("'{0}' is not a language tag")]
    Malformed(String),
}

/// A BCP 47 language tag, held in its canonical case.
///
/// Validated on construction rather than at the point of formatting — a tag
/// that reaches `Intl.NumberFormat` and throws there is a stack trace where a
/// settings-form error belonged. "Parse, don't validate": every value of this
/// type is a tag a formatter will accept.
///
/// ```
/// use afisharr_core::locale::LocaleTag;
///
/// assert_eq!(LocaleTag::parse("EN-gb").expect("a valid tag").as_str(), "en-GB");
/// assert!(LocaleTag::parse("en_GB").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct LocaleTag(String);

impl LocaleTag {
    /// Parses and canonicalises a language tag.
    ///
    /// Canonical case is lowercase language, title-case script, uppercase
    /// region, which is what the ECMA-402 formatters and the CSS `:lang`
    /// selector both expect.
    ///
    /// # Errors
    /// Returns [`LocaleTagError::Malformed`] when the text is not a sequence of
    /// one to eight alphanumeric subtags separated by hyphens.
    pub fn parse(text: &str) -> Result<Self, LocaleTagError> {
        let malformed = || LocaleTagError::Malformed(text.to_owned());
        if text.is_empty() || text.len() > 35 {
            return Err(malformed());
        }

        let mut canonical = String::with_capacity(text.len());
        for (index, subtag) in text.split('-').enumerate() {
            if subtag.is_empty()
                || subtag.len() > 8
                || !subtag.chars().all(|c| c.is_ascii_alphanumeric())
            {
                return Err(malformed());
            }
            // The first subtag carries its own rule, and it is the one that
            // decides whether a formatter will take the tag at all. BCP 47's
            // primary language subtag is two or three letters, or five to
            // eight; one letter, four letters, and anything with a digit in it
            // are reserved or simply not language subtags. Accepting them made
            // this type's promise false: `engl`, `e`, and `123` all parsed,
            // were stored in `instance.locale`, reached
            // `new Intl.PluralRules(locale)` in the browser, and threw the
            // `RangeError` this validation exists to turn into a settings-form
            // refusal.
            if index == 0 && !is_language_subtag(subtag) {
                return Err(malformed());
            }
            if index > 0 {
                canonical.push('-');
            }
            canonical.push_str(&canonicalise(index, subtag));
        }
        Ok(Self(canonical))
    }

    /// The tag as stored and as sent to a formatter.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for LocaleTag {
    fn default() -> Self {
        Self(DEFAULT.to_owned())
    }
}

impl fmt::Display for LocaleTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for LocaleTag {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// Whether `subtag` is a primary language subtag (BCP 47 §2.2.1).
///
/// `alpha{2,3}` is every living language and every ISO 639-2/5 code;
/// `alpha{5,8}` is the registered-language range. Four letters are reserved and
/// name nothing, one letter is a singleton that only ever introduces an
/// extension, and a digit cannot appear at all.
fn is_language_subtag(subtag: &str) -> bool {
    matches!(subtag.len(), 2..=3 | 5..=8) && subtag.bytes().all(|byte| byte.is_ascii_alphabetic())
}

/// Subtag case, by position: language, script, then everything else.
fn canonicalise(index: usize, subtag: &str) -> String {
    match index {
        0 => subtag.to_ascii_lowercase(),
        1 if subtag.len() == 4 => {
            let mut out = subtag.to_ascii_lowercase();
            out[..1].make_ascii_uppercase();
            out
        }
        _ if subtag.len() <= 3 => subtag.to_ascii_uppercase(),
        _ => subtag.to_ascii_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_catalogue_that_always_ships() {
        assert_eq!(LocaleTag::default().as_str(), DEFAULT);
    }

    #[test]
    fn a_language_only_tag_parses() {
        assert_eq!(LocaleTag::parse("en").expect("valid").as_str(), "en");
    }

    #[test]
    fn case_is_canonicalised_by_subtag_position() {
        let cases = [
            ("en-gb", "en-GB"),
            ("ZH-hant-tw", "zh-Hant-TW"),
            ("PT-br", "pt-BR"),
        ];
        for (input, expected) in cases {
            assert_eq!(LocaleTag::parse(input).expect("valid").as_str(), expected);
        }
    }

    #[test]
    fn an_underscore_separator_is_refused_rather_than_repaired() {
        // Repairing it would accept a POSIX locale name and store something the
        // operator did not type, which is the substitution P6 rejects.
        assert!(LocaleTag::parse("en_GB").is_err());
    }

    #[test]
    fn empty_and_over_long_subtags_are_refused() {
        for malformed in ["", "en-", "-en", "en--GB", "englishlanguage"] {
            assert!(
                LocaleTag::parse(malformed).is_err(),
                "{malformed} must not parse"
            );
        }
    }

    #[test]
    fn the_tag_round_trips_through_json() {
        let tag = LocaleTag::parse("en-GB").expect("valid");
        let encoded = serde_json::to_string(&tag).expect("serialises");
        assert_eq!(encoded, "\"en-GB\"");
        assert_eq!(
            serde_json::from_str::<LocaleTag>(&encoded).expect("deserialises"),
            tag
        );
    }

    #[test]
    fn a_malformed_tag_fails_deserialisation_naming_itself() {
        let error = serde_json::from_str::<LocaleTag>("\"en_GB\"")
            .expect_err("a malformed tag must not deserialise");
        assert!(error.to_string().contains("en_GB"), "{error}");
    }
}
