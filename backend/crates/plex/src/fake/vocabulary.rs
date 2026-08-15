// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The filter vocabulary the fake declares about itself.
//!
//! Small on purpose. The point is not to mirror a real server's field list —
//! that changes every release, and the contract test is what checks the shape.
//! The point is that the vocabulary is *discovered*: a filter carries the
//! endpoint its choices come from, a field carries the type its operators are
//! looked up by, and the operator list is the server's rather than a
//! compiled-in allowlist (PRD §13.2.4).
//!
//! **Two things a real server does that this did not.** It answers `Meta.Type`
//! as every libtype the section filters rather than as the one the caller
//! asked about — a client picks its own libtype out of that list
//! (`plexapi/library.py:2674`), and a list of one only ever answers the caller
//! that guessed right. And it answers a *short* `Meta` unless the request also
//! carries `includeAdvanced=1` (`plexapi/library.py:890`), so a fake that
//! always answered the full one hid a client that would get less from a real
//! server.

use crate::fake::element::Element;

/// The genres the fake's items carry, with the keys a filter matches on.
///
/// Keyed as well as titled because that is how Plex filters: a client resolves
/// a genre's name to its key through the choice list and sends the key
/// (`plexapi/library.py:1178`), so a fake matching on the name would pass a
/// client that skipped the resolution.
pub(crate) const GENRES: [(&str, &str); 3] =
    [("93", "Comedy"), ("94", "Drama"), ("95", "Science Fiction")];

/// One filter a libtype offers.
struct FilterSpec {
    name: &'static str,
    filter_type: &'static str,
    title: &'static str,
    /// Whether the filter declares an endpoint its choices come from.
    enumerated: bool,
}

/// The filters one libtype offers.
fn filters(libtype: &str) -> Vec<FilterSpec> {
    if libtype == "collection" {
        return vec![FilterSpec {
            name: "label",
            filter_type: "string",
            title: "Label",
            enumerated: true,
        }];
    }
    vec![
        FilterSpec {
            name: "genre",
            filter_type: "string",
            title: "Genre",
            enumerated: true,
        },
        FilterSpec {
            name: "year",
            filter_type: "integer",
            title: "Year",
            enumerated: false,
        },
    ]
}

/// The numeric `type` Plex's query arguments take for a libtype.
pub(crate) const fn plex_type(libtype: &str) -> u8 {
    match libtype.as_bytes() {
        b"show" => 2,
        b"season" => 3,
        b"episode" => 4,
        b"artist" => 8,
        b"album" => 9,
        b"track" => 10,
        b"photoalbum" => 12,
        b"photo" => 13,
        b"collection" => 18,
        _ => 1,
    }
}

/// Every libtype a section of this kind filters.
///
/// A show library filters shows, seasons, and episodes; a music library
/// filters artists, albums, and tracks. A client that asked about seasons and
/// got the show entry would be filtering a type it never queried.
pub(crate) fn libtypes_of(kind: &str) -> &'static [&'static str] {
    match kind.as_bytes() {
        b"show" => &["show", "season", "episode"],
        b"artist" => &["artist", "album", "track"],
        b"photo" => &["photoalbum", "photo"],
        _ => &["movie"],
    }
}

/// The `Meta` block, for the libtypes an endpoint describes.
///
/// `advanced` is what `includeAdvanced=1` asks for: the field list and the
/// operator table. Without it a real server answers the filter and sort lists
/// alone, and a client that never sent the argument would be building custom
/// filters out of a field list it did not have.
pub(crate) fn describe(section: &str, libtypes: &[&str], advanced: bool) -> Element {
    let mut meta = Element::named("Meta").singular().children(
        libtypes
            .iter()
            .map(|libtype| filtering_type(section, libtype, advanced)),
    );
    if advanced {
        meta = meta.children(field_types());
    }
    meta
}

/// One libtype's filters, sorts, and — when asked — its fields.
fn filtering_type(section: &str, libtype: &str, advanced: bool) -> Element {
    let plex = plex_type(libtype);
    let mut entry = Element::named("Type")
        .text(
            "key",
            format!("/library/sections/{section}/all?type={plex}"),
        )
        .text("type", libtype.to_owned())
        .text("title", libtype.to_owned())
        .flag("active", false)
        .children(filters(libtype).into_iter().map(|filter| {
            let declared = Element::named("Filter")
                .text("filter", filter.name)
                .text("filterType", filter.filter_type)
                .text("title", filter.title)
                .text("type", "filter");
            if filter.enumerated {
                // The server composes this, query string and all, and the type
                // it carries is the libtype the filter belongs to. The route
                // that answers it is the one named here — nothing reassembles
                // it from parts (P7).
                return declared.text(
                    "key",
                    format!("/library/sections/{section}/{}?type={plex}", filter.name),
                );
            }
            declared
        }))
        .children(sorts(section, libtype));
    if advanced {
        entry = entry.children(fields(libtype));
    }
    entry
}

/// The sorts one libtype offers.
fn sorts(section: &str, libtype: &str) -> Vec<Element> {
    [
        ("titleSort", "Title", "asc"),
        ("addedAt", "Date Added", "desc"),
        ("year", "Year", "desc"),
    ]
    .into_iter()
    .map(|(key, title, direction)| {
        Element::named("Sort")
            .text("key", key)
            .text("title", title)
            .text("defaultDirection", direction)
            .text("descKey", format!("{key}:desc"))
            .text(
                "firstCharacterKey",
                format!("/library/sections/{section}/firstCharacter"),
            )
            .flag("default", key == "titleSort" && libtype != "collection")
    })
    .collect()
}

/// The fields one libtype declares, under the dotted keys a real server uses.
///
/// Dotted because a client sends the key back verbatim as the filter argument
/// (`plexapi/library.py:1082`), so a bare `genre` here is a fake describing a
/// query nobody sends.
fn fields(libtype: &str) -> Vec<Element> {
    let mut declared = vec![
        ("title", "string", None, "Title"),
        ("year", "integer", None, "Year"),
        ("userRating", "integer", Some("rating"), "Rating"),
        ("label", "tag", None, "Label"),
    ];
    if libtype != "collection" {
        declared.push(("genre", "tag", None, "Genre"));
        declared.push(("audioLanguage", "string", None, "Audio Language"));
    }
    declared
        .into_iter()
        .map(|(key, field_type, sub_type, title)| {
            Element::named("Field")
                .text("key", format!("{libtype}.{key}"))
                .text("type", field_type)
                .text("title", title)
                .maybe_text("subType", sub_type)
        })
        .collect()
}

/// The operator table, indexed by field type.
fn field_types() -> Vec<Element> {
    [
        ("tag", &["=", "!="][..], &["is", "is not"][..]),
        (
            "integer",
            &["=", "!=", ">>=", "<<="],
            &["is", "is not", "is at least", "is at most"],
        ),
        (
            "string",
            &["=", "==", "!=", "!=="],
            &["contains", "is", "does not contain", "is not"],
        ),
        ("boolean", &["="], &["is"]),
        ("date", &[">>=", "<<="], &["is after", "is before"]),
    ]
    .into_iter()
    .map(|(field_type, keys, titles)| {
        Element::named("FieldType")
            .text("type", field_type)
            .children(keys.iter().zip(titles.iter()).map(|(key, title)| {
                Element::named("Operator")
                    .text("key", *key)
                    .text("title", *title)
            }))
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::json;

    // These assertions are about the *shape* the fake emits, and stop there.
    // Whether the client can read it is proved end to end against the running
    // server and the real parsers; whether it matches a real Plex is the
    // contract test's job, and whether an independent client can read it is
    // the reference cross-check's.

    fn meta(kind: &str, advanced: bool) -> serde_json::Value {
        json::document(&describe("1", libtypes_of(kind), advanced))["Meta"].clone()
    }

    #[test]
    fn a_section_declares_every_libtype_it_filters_rather_than_one() {
        // A client picks its own libtype out of this list, and a list of one
        // only ever answers the caller that guessed right.
        let shows = meta("show", true);
        let declared: Vec<&str> = shows["Type"]
            .as_array()
            .expect("a type list")
            .iter()
            .filter_map(|entry| entry["type"].as_str())
            .collect();
        assert_eq!(declared, ["show", "season", "episode"]);
        assert_eq!(
            meta("movie", true)["Type"].as_array().map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn a_short_meta_is_what_a_request_without_the_advanced_argument_gets() {
        // A fake that always answered the full block hid a client that would
        // get a short one from a real server (`plexapi/library.py:890`).
        let short = meta("movie", false);
        assert!(short.get("FieldType").is_none());
        assert!(short["Type"][0].get("Field").is_none());
        assert!(short["Type"][0].get("Filter").is_some());
    }

    #[test]
    fn a_field_carries_the_dotted_key_a_client_sends_back() {
        let fields = meta("movie", true)["Type"][0]["Field"].clone();
        let keys: Vec<&str> = fields
            .as_array()
            .expect("a field list")
            .iter()
            .filter_map(|field| field["key"].as_str())
            .collect();
        assert!(keys.contains(&"movie.genre"), "{keys:?}");
        assert!(keys.contains(&"movie.userRating"), "{keys:?}");
    }

    #[test]
    fn a_filter_with_choices_declares_the_endpoint_they_come_from() {
        let filters = meta("movie", true)["Type"][0]["Filter"].clone();
        assert_eq!(filters[0]["key"], "/library/sections/1/genre?type=1");
        // A free-value filter declares none, and a client that assumed every
        // filter has a choice list would request the server root.
        assert!(filters[1].get("key").is_none());
    }

    #[test]
    fn the_collection_libtype_declares_its_own_filters() {
        let collections = meta_of_collection();
        assert_eq!(collections["Type"][0]["type"], "collection");
        assert_eq!(collections["Type"][0]["Filter"][0]["filter"], "label");
        assert_eq!(
            collections["Type"][0]["Filter"][0]["key"],
            "/library/sections/1/label?type=18"
        );
    }

    fn meta_of_collection() -> serde_json::Value {
        json::document(&describe("1", &["collection"], true))["Meta"].clone()
    }

    #[test]
    fn the_operator_table_differs_by_field_type() {
        let table = meta("movie", true)["FieldType"].clone();
        let operators = |index: usize| {
            table[index]["Operator"]
                .as_array()
                .expect("an operator list")
                .iter()
                .filter_map(|operator| operator["key"].as_str())
                .map(str::to_owned)
                .collect::<Vec<String>>()
        };
        assert_eq!(table[0]["type"], "tag");
        assert_eq!(operators(0), ["=", "!="]);
        assert_eq!(table[1]["type"], "integer");
        assert!(
            operators(1).contains(&">>=".to_owned()),
            "{:?}",
            operators(1)
        );
        assert!(
            operators(2).contains(&"==".to_owned()),
            "{:?}",
            operators(2)
        );
    }

    #[test]
    fn every_libtype_maps_to_the_number_plexs_queries_take() {
        assert_eq!(plex_type("movie"), 1);
        assert_eq!(plex_type("show"), 2);
        assert_eq!(plex_type("collection"), 18);
        assert_eq!(plex_type("track"), 10);
    }
}
