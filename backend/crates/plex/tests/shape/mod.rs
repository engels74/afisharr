// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Comparing the *shape* of two JSON answers, ignoring their content.
//!
//! The contract test asks one question — does the fake assume a response shape
//! a real Plex does not produce? — and the answer must not depend on a real
//! server having the same films in it. So both answers are reduced to a set of
//! `path: type` facts, and the comparison is over those.

use std::collections::BTreeSet;

use serde_json::Value;

/// Every `path: type` fact in one document.
///
/// Array indices collapse to `[]`, so an answer with two items and an answer
/// with two hundred have the same shape. Every element still contributes, so an
/// optional field present on one item in fifty is still part of the shape.
#[must_use]
pub fn of(document: &Value) -> BTreeSet<String> {
    let mut facts = BTreeSet::new();
    walk("", document, &mut facts);
    facts
}

fn walk(path: &str, value: &Value, facts: &mut BTreeSet<String>) {
    match value {
        Value::Object(fields) => {
            for (name, field) in fields {
                walk(&format!("{path}.{name}"), field, facts);
            }
            if fields.is_empty() {
                facts.insert(format!("{path}: object"));
            }
        }
        Value::Array(items) => {
            for item in items {
                walk(&format!("{path}[]"), item, facts);
            }
            if items.is_empty() {
                facts.insert(format!("{path}: array"));
            }
        }
        // `null` is its own type rather than the type it would have carried:
        // a field a real server sends as `null` and the fake sends as a string
        // is a difference a client can trip over, and it should show up here.
        Value::Null => {
            facts.insert(format!("{path}: null"));
        }
        Value::Bool(_) => {
            facts.insert(format!("{path}: boolean"));
        }
        Value::Number(_) => {
            facts.insert(format!("{path}: number"));
        }
        Value::String(_) => {
            facts.insert(format!("{path}: string"));
        }
    }
}

/// What the fake claims that the real answer does not support.
///
/// Directional on purpose. A real server carrying fields the fake omits is
/// fine — the fake is not an emulator, and PRD §21.10.2 says so. A fake
/// carrying a field, or a type, the real server does not is the drift that
/// makes every test written against it a test of a server that does not exist.
#[must_use]
pub fn unsupported_claims(fake: &Value, real: &Value) -> Vec<String> {
    let real = of(real);
    of(fake)
        .into_iter()
        .filter(|fact| !real.contains(fact))
        .collect()
}

/// What the real answer carries that the fake does not, minus what is allowed.
///
/// The other direction, and the one that passed every omission the reference
/// audit found: a fake that answers *less* than a real server is a fake whose
/// silences are invisible. The fake is not an emulator (PRD section 21.10.2),
/// so some omissions are deliberate, and a deliberate omission is a line
/// somebody wrote in the allow-list with a reason rather than a silence.
///
/// An allowance is a path prefix, matched against the fact's path. It never
/// matches a *type*: allowing `.MediaContainer.Metadata[].Chapter` says the
/// fake does not model chapters, not that it may send one of the wrong type.
#[must_use]
pub fn missing_claims(fake: &Value, real: &Value, allowed: &[&str]) -> Vec<String> {
    let fake = of(fake);
    of(real)
        .into_iter()
        .filter(|fact| !fake.contains(fact))
        .filter(|fact| {
            let path = fact.split(':').next().unwrap_or(fact);
            !allowed.iter().any(|allowance| path.starts_with(allowance))
        })
        .collect()
}

/// Panics naming the call when the fake claims anything the real answer does not.
///
/// The message names the call and the exact facts, because a release-lane
/// failure that said only "shape drift" would send whoever reads it back to a
/// server they may not have.
pub fn assert_supported(call: &str, fake: &Value, real: &Value) {
    let unsupported = unsupported_claims(fake, real);
    assert!(
        unsupported.is_empty(),
        "contract drift on {call}: the fake answers fields a real Plex does not, \
         so every test written against them tests a server that does not exist.\n  {}",
        unsupported.join("\n  ")
    );
}

/// Panics naming the call when the real answer carries something unaccounted for.
///
/// The message asks for a decision rather than a fix: either the fake owes the
/// field an answer, or the allow-list owes it a line saying why not.
pub fn assert_covered(call: &str, fake: &Value, real: &Value, allowed: &[&str]) {
    let missing = missing_claims(fake, real, allowed);
    assert!(
        missing.is_empty(),
        "contract gap on {call}: a real Plex answers fields the fake does not, and none of \
         them is on the allow-list. Either the fake should answer them, or add each to the \
         allow-list with the reason it is not modelled.\n  {}",
        missing.join("\n  ")
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // These documents are invented, and they are a test of this comparator
    // rather than a claim about any real server. The real answers live in the
    // release lane, where they come from a real Plex.

    #[test]
    fn two_documents_with_the_same_fields_have_the_same_shape() {
        let one = json!({"MediaContainer": {"size": 1, "Metadata": [{"ratingKey": "1"}]}});
        let other = json!({"MediaContainer": {"size": 9, "Metadata": [{"ratingKey": "42"}]}});
        assert_eq!(of(&one), of(&other));
        assert!(unsupported_claims(&one, &other).is_empty());
    }

    #[test]
    fn array_length_is_not_part_of_the_shape() {
        let one = json!({"Metadata": [{"ratingKey": "1"}]});
        let many = json!({"Metadata": [{"ratingKey": "1"}, {"ratingKey": "2"}]});
        assert_eq!(of(&one), of(&many));
    }

    #[test]
    fn every_element_contributes_so_an_occasional_field_still_counts() {
        // The field that appears on one item in fifty is exactly the one a fake
        // is most likely to be wrong about.
        let mixed = json!({"Metadata": [{"ratingKey": "1"}, {"ratingKey": "2", "titleSort": "A"}]});
        assert!(of(&mixed).contains(".Metadata[].titleSort: string"));
    }

    #[test]
    fn a_field_only_the_fake_sends_is_reported_as_drift() {
        let fake = json!({"MediaContainer": {"machineIdentifier": "x", "inventedField": true}});
        let real = json!({"MediaContainer": {"machineIdentifier": "abc"}});
        assert_eq!(
            unsupported_claims(&fake, &real),
            [".MediaContainer.inventedField: boolean"]
        );
    }

    #[test]
    fn a_type_the_fake_gets_wrong_is_reported_as_drift() {
        // The failure this exists for: Plex sends `childCount` as a string on
        // some builds, and a fake that sent a number would let a client that
        // only handles numbers pass here and fail in the field.
        let fake = json!({"Metadata": [{"childCount": 12}]});
        let real = json!({"Metadata": [{"childCount": "12"}]});
        assert_eq!(
            unsupported_claims(&fake, &real),
            [".Metadata[].childCount: number"]
        );
    }

    #[test]
    fn a_field_only_the_real_server_sends_is_not_drift_in_that_direction() {
        // The fake is not an emulator (PRD §21.10.2). It answers the surface
        // this crate calls and is allowed to be wrong about everything else.
        let fake = json!({"MediaContainer": {"machineIdentifier": "x"}});
        let real = json!({"MediaContainer": {"machineIdentifier": "abc", "myPlex": true}});
        assert!(unsupported_claims(&fake, &real).is_empty());
    }

    #[test]
    fn a_field_only_the_real_server_sends_is_a_gap_until_somebody_allows_it() {
        // The assertion that was missing, and the one every omission the
        // reference audit found would have passed.
        let fake = json!({"MediaContainer": {"machineIdentifier": "x"}});
        let real = json!({"MediaContainer": {"machineIdentifier": "abc", "myPlex": true}});
        assert_eq!(
            missing_claims(&fake, &real, &[]),
            [".MediaContainer.myPlex: boolean"]
        );
        assert!(missing_claims(&fake, &real, &[".MediaContainer.myPlex"]).is_empty());
    }

    #[test]
    fn an_allowance_covers_everything_under_it() {
        // A subtree the fake does not model is one line, not one line per leaf.
        let fake = json!({"Metadata": [{"ratingKey": "1"}]});
        let real = json!({"Metadata": [{"ratingKey": "1", "Chapter": [{"id": 1, "tag": "One"}]}]});
        assert_eq!(missing_claims(&fake, &real, &[]).len(), 2);
        assert!(missing_claims(&fake, &real, &[".Metadata[].Chapter"]).is_empty());
    }

    #[test]
    fn a_null_where_a_value_was_expected_is_reported() {
        let fake = json!({"thumb": "/library/metadata/1/thumb/17"});
        let real = json!({"thumb": null});
        assert_eq!(unsupported_claims(&fake, &real), [".thumb: string"]);
    }

    #[test]
    #[should_panic(expected = "contract drift on GET /identity")]
    fn the_assertion_names_the_call_it_failed_on() {
        assert_supported(
            "GET /identity",
            &json!({"MediaContainer": {"inventedField": 1}}),
            &json!({"MediaContainer": {"machineIdentifier": "abc"}}),
        );
    }
}
