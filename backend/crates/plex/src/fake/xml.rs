// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Rendering one described answer as the XML a Plex server answers by default.
//!
//! This is what a real Plex sends to a client that asks for nothing in
//! particular, and until now the fake had never produced a byte of it — so
//! every claim about the surface was checked only by readers written in this
//! repository, against JSON this repository also wrote.
//!
//! Written out rather than taken from a serialiser crate, and for the reason
//! the seed generator is: the output is an *assertion*, compared byte for byte
//! by tests and read by an external client, and a dependency that reformats
//! its output in a patch release would change what those tests mean. Twenty
//! lines of escaping have no such release note.

use crate::fake::element::Element;

/// The whole document, with the declaration a real server sends.
pub(crate) fn document(element: &Element) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    write_element(element, &mut out);
    out
}

/// Writes one element and everything under it.
fn write_element(element: &Element, out: &mut String) {
    let tag = element.tag().xml();
    out.push('<');
    out.push_str(tag);
    for (name, value) in element.attributes() {
        out.push(' ');
        out.push_str(name);
        out.push_str("=\"");
        escape(&value.as_text(), out);
        out.push('"');
    }
    if element.child_elements().is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for child in element.child_elements() {
        write_element(child, out);
    }
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

/// Escapes one attribute value.
///
/// The five predefined entities, and then the control characters XML 1.0 has
/// no representation for at all — a parser meeting one refuses the whole
/// document, so a title holding a stray byte would take the library with it.
/// Dropped rather than escaped, because there is no escape that works.
fn escape(value: &str, out: &mut String) {
    for character in value.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(character),
            control if control.is_control() => {}
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_element_with_no_children_is_written_empty() {
        let rendered = document(&Element::named("MediaContainer").number("size", 0_i64));
        assert_eq!(
            rendered,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<MediaContainer size=\"0\"/>"
        );
    }

    #[test]
    fn a_content_row_is_written_under_its_xml_name() {
        // `Video`, not `Metadata`: the JSON name would be unreadable to a
        // client that resolves its classes by element tag.
        let rendered = document(
            &Element::named("MediaContainer")
                .child(Element::content("Video").text("title", "Alien")),
        );
        assert!(rendered.contains("<Video title=\"Alien\"/>"), "{rendered}");
        assert!(rendered.ends_with("</MediaContainer>"), "{rendered}");
    }

    #[test]
    fn a_flag_is_written_as_one_and_zero() {
        let rendered = document(&Element::named("Hub").flag("deletable", false));
        assert!(rendered.contains("deletable=\"0\""), "{rendered}");
    }

    #[test]
    fn the_five_predefined_entities_are_escaped() {
        let rendered = document(&Element::content("Video").text("title", r#"Tom & "Jerry" <'>"#));
        assert!(
            rendered.contains("title=\"Tom &amp; &quot;Jerry&quot; &lt;&apos;&gt;\""),
            "{rendered}"
        );
    }

    #[test]
    fn a_control_character_is_dropped_rather_than_escaped() {
        // XML 1.0 has no representation for one, and a parser meeting it
        // refuses the whole document — so a single stray byte in one title
        // would take the entire library answer with it.
        let rendered = document(&Element::content("Video").text("title", "Ali\u{0}en"));
        assert!(rendered.contains("title=\"Alien\""), "{rendered}");
    }

    #[test]
    fn nesting_survives_two_levels_down() {
        let rendered = document(&Element::named("MediaContainer").child(
            Element::content("Video").child(Element::named("Media").child(Element::named("Part"))),
        ));
        assert!(
            rendered.contains("<Video><Media><Part/></Media></Video>"),
            "{rendered}"
        );
    }
}
