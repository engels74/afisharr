// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! How Plex spells a value on the wire, and how this build reads each spelling.
//!
//! Every field in a Plex answer is an XML attribute underneath, and XML has one
//! type: text. What arrives as JSON is a translation, and the translation is
//! not consistent — `1`, `"1"`, and `true` all appear where a flag belongs, and
//! `12` and `"12"` both appear where a count does. A field typed strictly
//! against one spelling does not read a wrong value; it fails the whole parse,
//! and takes every other fact in the answer with it.
//!
//! So the two spellings live here, once, rather than as a `bool` in one module
//! and a permissive reader in another (P7).

use serde::{Deserialize, Deserializer};

/// A value Plex spells as a number in one version and a string in another.
///
/// Not defensive typing for its own sake: `childCount` arrives as `"12"` from
/// some builds and `12` from others, and a client that accepts only one of them
/// breaks on a server upgrade nobody here controls.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum StringOrNumber {
    /// Spelled as a JSON number.
    Number(i64),
    /// Spelled as a JSON string.
    Text(String),
}

impl StringOrNumber {
    /// The value as an integer, or `None` when it is neither spelling.
    pub(crate) fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Text(text) => text.trim().parse().ok(),
        }
    }

    /// The value as the text it was sent as.
    ///
    /// For the values that are *identifiers* rather than counts. A rating key
    /// read through [`Self::as_i64`] and re-rendered would be normalised —
    /// somebody else's opaque identifier rewritten by this build — and any
    /// spelling that did not parse would come back as "absent" (P4).
    pub(crate) fn into_text(self) -> String {
        match self {
            Self::Number(value) => value.to_string(),
            Self::Text(text) => text,
        }
    }
}

/// One Plex flag, in whichever of the four spellings it arrived as.
///
/// `true`, `1`, `"1"`, and `"true"` are all set; `false`, `0`, `"0"`, `"false"`
/// and an empty string are all clear. The same cast a reference client applies
/// to every boolean attribute it reads (`plexapi/utils.py:173-178`), which is
/// evidence that a real server uses more than one of them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Flag(bool);

impl Flag {
    /// Whether the flag is set.
    pub(crate) const fn is_set(self) -> bool {
        self.0
    }
}

impl From<bool> for Flag {
    fn from(value: bool) -> Self {
        Self(value)
    }
}

impl<'de> Deserialize<'de> for Flag {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Spelling {
            Bool(bool),
            Number(i64),
            Text(String),
        }

        Ok(Self(match Spelling::deserialize(deserializer)? {
            Spelling::Bool(value) => value,
            Spelling::Number(value) => value != 0,
            // Anything a server sends that is not one of the four is *not set*
            // rather than an error: a flag this build cannot read is a fact it
            // was not given, and failing the parse would lose the answer's
            // other twenty fields over one attribute (P1).
            Spelling::Text(text) => matches!(text.trim(), "1" | "true" | "True"),
        }))
    }
}

/// Reads one flag into an `Option<bool>`, keeping absence distinct from false.
///
/// The distinction is the whole point for `Part.accessible`: absent is "Plex
/// did not look", and `false` is "Plex looked and the file is not readable"
/// (P1).
pub(crate) fn optional_flag<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<bool>, D::Error> {
    Ok(Option::<Flag>::deserialize(deserializer)?.map(Flag::is_set))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag(json: &str) -> bool {
        serde_json::from_str::<Flag>(json)
            .expect("every spelling parses")
            .is_set()
    }

    #[test]
    fn every_spelling_a_real_server_uses_reads_as_the_same_flag() {
        assert!(flag("true"));
        assert!(flag("1"));
        assert!(flag(r#""1""#));
        assert!(flag(r#""true""#));
    }

    #[test]
    fn every_spelling_of_the_cleared_flag_reads_as_cleared() {
        assert!(!flag("false"));
        assert!(!flag("0"));
        assert!(!flag(r#""0""#));
        assert!(!flag(r#""false""#));
        assert!(!flag(r#""""#));
    }

    #[test]
    fn a_spelling_this_build_does_not_know_is_unset_rather_than_a_failed_parse() {
        // Losing the whole item over one attribute is the failure mode; a flag
        // it could not read is a fact it was not given.
        assert!(!flag(r#""yes""#));
    }

    #[test]
    fn a_count_reads_the_same_from_either_spelling() {
        let number: StringOrNumber = serde_json::from_str("12").expect("parses");
        let text: StringOrNumber = serde_json::from_str(r#""12""#).expect("parses");
        assert_eq!(number.as_i64(), Some(12));
        assert_eq!(text.as_i64(), Some(12));
    }

    #[test]
    fn an_identifier_keeps_the_text_it_arrived_as() {
        let numeric: StringOrNumber = serde_json::from_str("5001").expect("parses");
        let odd: StringOrNumber = serde_json::from_str(r#""5001a""#).expect("parses");
        assert_eq!(numeric.into_text(), "5001");
        assert_eq!(odd.into_text(), "5001a");
    }
}
