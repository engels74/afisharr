// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One description of an answer, rendered two ways.
//!
//! A Plex Media Server speaks XML and translates it to JSON on request, so the
//! two renderings are not two formats a fake may write independently — they are
//! one document seen twice. Written by hand as two shapes they drift, and the
//! drift is invisible: the JSON half is what this repository's own client
//! reads, so a field forgotten in the XML half fails only under a reader nobody
//! here wrote.
//!
//! So every answer is described once, as an [`Element`], and
//! [`crate::fake::json`] and [`crate::fake::xml`] render that description.
//! Adding a field is one call on the builder and it appears in both.
//!
//! **Where the two renderings genuinely differ.** Plex's JSON is not a
//! mechanical copy of its XML, and two differences are load-bearing:
//!
//! - A content row is `<Video>`, `<Directory>`, `<Track>`, or `<Photo>` in XML
//!   and `Metadata` in JSON. [`Tag::Split`] carries both names.
//! - Repeated children become a JSON array under the tag name; a child that
//!   occurs once and is addressed as an object — `Meta` — is
//!   [`Element::singular`].

/// One attribute value, in the spellings Plex's wire format has.
///
/// XML has one type and it is text, so every variant renders as text there.
/// JSON is where the difference shows, and where a fake gets it wrong: a flag
/// spelled `true` in one answer and `"1"` in another is two shapes for one
/// fact, and a client written against the first fails on the second.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Attribute {
    /// A string.
    Text(String),
    /// A whole number.
    Number(i64),
    /// A decimal, e.g. an aspect ratio.
    Decimal(f64),
    /// A flag. Rendered `1`/`0` in both renderings, which is the one spelling
    /// a reference client's cast accepts everywhere (`plexapi/utils.py:173`).
    Flag(bool),
}

impl Attribute {
    /// The value as the text an XML attribute carries.
    pub(crate) fn as_text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Number(number) => number.to_string(),
            Self::Decimal(number) => number.to_string(),
            Self::Flag(flag) => i32::from(*flag).to_string(),
        }
    }
}

/// What an element is called, in each rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tag {
    /// One name in both renderings.
    Same(&'static str),
    /// Two names: the XML element, and the JSON key.
    ///
    /// A content row is the case. Plex sends `<Video type="movie">` in XML and
    /// `"Metadata": [{"type": "movie"}]` in JSON, and a fake that used one name
    /// for both would be unreadable by half the clients in the world.
    Split {
        /// The XML element name.
        xml: &'static str,
        /// The JSON key.
        json: &'static str,
    },
}

impl Tag {
    /// The XML element name.
    pub(crate) const fn xml(self) -> &'static str {
        match self {
            Self::Same(name) | Self::Split { xml: name, .. } => name,
        }
    }

    /// The JSON key.
    pub(crate) const fn json(self) -> &'static str {
        match self {
            Self::Same(name) | Self::Split { json: name, .. } => name,
        }
    }
}

/// One described element: a tag, its attributes, and its children.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Element {
    tag: Tag,
    singular: bool,
    attributes: Vec<(&'static str, Attribute)>,
    children: Vec<Element>,
}

impl Element {
    /// An element with one name in both renderings.
    pub(crate) const fn named(tag: &'static str) -> Self {
        Self {
            tag: Tag::Same(tag),
            singular: false,
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    /// A content row: one name in XML, `Metadata` in JSON.
    pub(crate) const fn content(xml: &'static str) -> Self {
        Self {
            tag: Tag::Split {
                xml,
                json: "Metadata",
            },
            singular: false,
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Marks this element as occurring once, so JSON renders it as an object
    /// rather than as a one-element array. `Meta` is the case.
    #[must_use]
    pub(crate) const fn singular(mut self) -> Self {
        self.singular = true;
        self
    }

    /// Sets a text attribute.
    #[must_use]
    pub(crate) fn text(self, name: &'static str, value: impl Into<String>) -> Self {
        self.set(name, Attribute::Text(value.into()))
    }

    /// Sets a numeric attribute.
    #[must_use]
    pub(crate) fn number(self, name: &'static str, value: impl Into<i64>) -> Self {
        self.set(name, Attribute::Number(value.into()))
    }

    /// Sets a decimal attribute.
    #[must_use]
    pub(crate) fn decimal(self, name: &'static str, value: f64) -> Self {
        self.set(name, Attribute::Decimal(value))
    }

    /// Sets a flag attribute.
    #[must_use]
    pub(crate) fn flag(self, name: &'static str, value: bool) -> Self {
        self.set(name, Attribute::Flag(value))
    }

    /// Sets a text attribute only when there is a value.
    ///
    /// Absence is a fact of its own here: an absent sort title is a missing
    /// attribute and an empty one is a value, and §15.6 turns on the
    /// difference (P1).
    #[must_use]
    pub(crate) fn maybe_text(self, name: &'static str, value: Option<impl Into<String>>) -> Self {
        match value {
            None => self,
            Some(value) => self.text(name, value),
        }
    }

    /// Sets a numeric attribute only when there is a value.
    #[must_use]
    pub(crate) fn maybe_number(self, name: &'static str, value: Option<impl Into<i64>>) -> Self {
        match value {
            None => self,
            Some(value) => self.number(name, value),
        }
    }

    /// Adds one child.
    #[must_use]
    pub(crate) fn child(mut self, child: Self) -> Self {
        self.children.push(child);
        self
    }

    /// Adds several children.
    #[must_use]
    pub(crate) fn children(mut self, children: impl IntoIterator<Item = Self>) -> Self {
        self.children.extend(children);
        self
    }

    /// Writes one attribute, replacing any earlier value under that name.
    ///
    /// Replacing rather than appending, because XML forbids a repeated
    /// attribute: a second `size` would make the answer unparseable by every
    /// reader rather than merely wrong.
    fn set(mut self, name: &'static str, value: Attribute) -> Self {
        match self
            .attributes
            .iter_mut()
            .find(|(existing, _)| *existing == name)
        {
            Some(slot) => slot.1 = value,
            None => self.attributes.push((name, value)),
        }
        self
    }

    /// This element's tag.
    pub(crate) const fn tag(&self) -> Tag {
        self.tag
    }

    /// Whether JSON renders this as an object rather than an array.
    pub(crate) const fn is_singular(&self) -> bool {
        self.singular
    }

    /// The attributes, in the order they were written.
    pub(crate) fn attributes(&self) -> &[(&'static str, Attribute)] {
        &self.attributes
    }

    /// The children, in the order they were added.
    pub(crate) fn child_elements(&self) -> &[Self] {
        &self.children
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_attribute_written_twice_keeps_the_last_value_and_appears_once() {
        // XML forbids a repeated attribute, so an appended second `size` would
        // make the whole answer unparseable rather than merely wrong.
        let element = Element::named("MediaContainer")
            .number("size", 0)
            .number("size", 7_i64);
        assert_eq!(element.attributes().len(), 1);
        assert_eq!(element.attributes()[0].1, Attribute::Number(7));
    }

    #[test]
    fn an_absent_value_writes_no_attribute_at_all() {
        let element = Element::content("Video").maybe_text("titleSort", None::<String>);
        assert!(element.attributes().is_empty());
    }

    #[test]
    fn a_content_row_carries_both_of_the_names_plex_gives_it() {
        // The one difference between the two renderings that is not a
        // formatting detail: XML says `Video`, JSON says `Metadata`.
        let row = Element::content("Video");
        assert_eq!(row.tag().xml(), "Video");
        assert_eq!(row.tag().json(), "Metadata");

        let hub = Element::named("Hub");
        assert_eq!(hub.tag().xml(), "Hub");
        assert_eq!(hub.tag().json(), "Hub");
    }

    #[test]
    fn every_attribute_kind_renders_as_the_text_an_xml_attribute_carries() {
        assert_eq!(Attribute::Text("a".to_owned()).as_text(), "a");
        assert_eq!(Attribute::Number(7).as_text(), "7");
        assert_eq!(Attribute::Decimal(1.78).as_text(), "1.78");
        assert_eq!(Attribute::Flag(true).as_text(), "1");
        assert_eq!(Attribute::Flag(false).as_text(), "0");
    }
}
