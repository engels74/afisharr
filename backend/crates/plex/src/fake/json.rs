// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Rendering one described answer as the JSON a Plex server translates to.
//!
//! Plex speaks XML and answers JSON when a client asks for it, so this is a
//! rendering of [`crate::fake::element::Element`] rather than a shape of its
//! own — the same description [`crate::fake::xml`] renders. Two hand-written
//! shapes would agree only as long as whoever edited one remembered the other.
//!
//! Two rules, and both come from what a real server's translation does:
//!
//! - Repeated children become an array under the tag's JSON name. `Metadata`,
//!   `Directory`, `Hub`, `Media`, `Part`, `Stream` are all arrays even when
//!   they hold one entry.
//! - A [`Element::singular`] child is one object rather than an array. `Meta`
//!   is the case, and a client reads it as `MediaContainer.Meta.Type`.

use serde_json::{Map, Value};

use crate::fake::element::{Attribute, Element};

/// The whole document: the element under its own JSON key.
pub(crate) fn document(element: &Element) -> Value {
    let mut root = Map::new();
    root.insert(element.tag().json().to_owned(), body(element));
    Value::Object(root)
}

/// One element's attributes and children, as a JSON object.
fn body(element: &Element) -> Value {
    let mut fields = Map::new();
    for (name, value) in element.attributes() {
        fields.insert((*name).to_owned(), attribute(value));
    }
    for child in element.child_elements() {
        let key = child.tag().json().to_owned();
        if child.is_singular() {
            fields.insert(key, body(child));
            continue;
        }
        match fields
            .entry(key)
            .or_insert_with(|| Value::Array(Vec::new()))
        {
            Value::Array(rows) => rows.push(body(child)),
            // Unreachable while every tag is either singular everywhere or
            // repeated everywhere, which the shape modules are what enforce.
            // Overwriting silently would drop a row; this keeps the answer
            // wrong in a way a test can see.
            other => *other = body(child),
        }
    }
    Value::Object(fields)
}

/// One attribute, in the JSON type it arrives as.
fn attribute(value: &Attribute) -> Value {
    match value {
        Attribute::Text(text) => Value::String(text.clone()),
        Attribute::Number(number) => Value::from(*number),
        Attribute::Decimal(number) => Value::from(*number),
        // `1` and `0`, never `true` and `false`. Every one of these is an XML
        // attribute on the wire, and one spelling everywhere is what stops a
        // client being written against whichever half of the answer it saw
        // first (`plexapi/utils.py:173-178`).
        Attribute::Flag(flag) => Value::from(i32::from(*flag)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_document_is_the_element_under_its_own_key() {
        let rendered = document(&Element::named("MediaContainer").number("size", 2_i64));
        assert_eq!(rendered["MediaContainer"]["size"], 2);
    }

    #[test]
    fn repeated_children_become_an_array_under_the_json_name() {
        // And the JSON name, not the XML one: a row Plex sends as `<Video>`
        // arrives in JSON as an entry of `Metadata`.
        let rendered = document(
            &Element::named("MediaContainer")
                .child(Element::content("Video").text("ratingKey", "1"))
                .child(Element::content("Video").text("ratingKey", "2")),
        );
        let rows = rendered["MediaContainer"]["Metadata"]
            .as_array()
            .expect("an array");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1]["ratingKey"], "2");
        assert!(rendered["MediaContainer"].get("Video").is_none());
    }

    #[test]
    fn a_single_child_is_still_an_array() {
        // A client that read one row as an object and two as an array would be
        // a client that works on a library with two films in it.
        let rendered = document(&Element::named("MediaContainer").child(Element::named("Hub")));
        assert!(rendered["MediaContainer"]["Hub"].is_array());
    }

    #[test]
    fn a_singular_child_is_an_object_rather_than_an_array() {
        let rendered = document(
            &Element::named("MediaContainer").child(
                Element::named("Meta")
                    .singular()
                    .child(Element::named("Type").text("type", "movie")),
            ),
        );
        assert_eq!(
            rendered["MediaContainer"]["Meta"]["Type"][0]["type"],
            "movie"
        );
    }

    #[test]
    fn a_flag_is_one_and_zero_rather_than_true_and_false() {
        // The spelling a reference client's cast accepts in every position
        // (`plexapi/utils.py:173-178`), and the one this fake sends everywhere.
        let rendered = document(
            &Element::named("MediaContainer")
                .flag("allowSync", true)
                .flag("refreshing", false),
        );
        assert_eq!(rendered["MediaContainer"]["allowSync"], 1);
        assert_eq!(rendered["MediaContainer"]["refreshing"], 0);
    }

    #[test]
    fn a_decimal_keeps_its_fraction() {
        let rendered = document(&Element::named("Media").decimal("aspectRatio", 1.78));
        assert!(
            (rendered["Media"]["aspectRatio"].as_f64().expect("a number") - 1.78).abs()
                < f64::EPSILON
        );
    }
}
