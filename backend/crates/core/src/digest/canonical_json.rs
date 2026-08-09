// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The canonical form a JSON body is digested over.

/// Re-serialises `body` with object keys sorted and no insignificant whitespace.
///
/// `serde_json`'s object map is ordered, so parsing and re-serialising is the
/// whole canonicalisation. Written as a named function anyway, because "the
/// digest is taken over this exact form" is a rule the schema depends on, and a
/// rule that lives only in the shape of a call is a rule that drifts.
///
/// # Errors
/// Returns the parse failure when `body` is not JSON.
///
/// ```
/// use afisharr_core::digest;
///
/// let a = digest::canonicalize(r#"{"b":1,"a":2}"#).expect("valid JSON");
/// let b = digest::canonicalize("{ \"a\" : 2 ,\n \"b\" : 1 }").expect("valid JSON");
/// assert_eq!(a, b);
/// ```
pub fn canonicalize(body: &str) -> Result<String, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(body)?;
    serde_json::to_string(&value)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn key_order_does_not_change_the_canonical_form() {
        assert_eq!(
            canonicalize(r#"{"kind":"Collection","name":"x"}"#).unwrap(),
            canonicalize(r#"{"name":"x","kind":"Collection"}"#).unwrap()
        );
    }

    #[test]
    fn nested_objects_are_canonicalised_too() {
        assert_eq!(
            canonicalize(r#"{"meta":{"b":1,"a":2}}"#).unwrap(),
            canonicalize(r#"{"meta":{"a":2,"b":1}}"#).unwrap()
        );
    }

    #[test]
    fn array_order_is_preserved_because_it_is_meaningful() {
        assert_ne!(
            canonicalize("[1,2]").unwrap(),
            canonicalize("[2,1]").unwrap()
        );
    }

    #[test]
    fn text_that_is_not_json_is_rejected() {
        assert!(canonicalize("not json").is_err());
    }
}
