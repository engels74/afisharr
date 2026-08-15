// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Applying one edit to one thing, whatever kind of thing it is.
//!
//! Plex has a single edit endpoint over every libtype:
//! `PUT /library/sections/{key}/all` writes whatever `id` names, at the libtype
//! `type` names (`plexapi/library.py:1743-1755`). The fake used to decide what
//! it was editing by looking for a `label` argument and routing everything else
//! to a collection, so an item's sort title could not be written at all — and
//! the sort-title round trip §15.6 requires had nothing to round-trip against.
//!
//! **Removals arrive comma-joined under one key.** `python-plexapi` sends
//! `label[].tag.tag-` once, holding every removed tag percent-quoted and joined
//! with commas (`plexapi/mixins/edit.py:331-333`). Read as one repeated key per
//! removal, a two-label removal removed one label and answered success.

use crate::fake::{
    request::Arguments,
    state::{FakeCollection, FakeItem},
};

/// The ids one edit names.
///
/// Comma-joined, because a real client joins every target into one argument
/// (`plexapi/library.py:1749`). Matching the whole string against one key made
/// a two-item edit write nothing and answer `{"size":1}`.
pub(crate) fn targets(arguments: &Arguments) -> Vec<String> {
    arguments
        .first("id")
        .map(|ids| {
            ids.split(',')
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The libtype an edit names, or `None` when it named none.
pub(crate) fn libtype(arguments: &Arguments) -> Option<u8> {
    arguments.first("type").and_then(|value| value.parse().ok())
}

/// The tags one edit adds under `tag`.
fn additions(arguments: &Arguments, tag: &str) -> Vec<String> {
    let prefix = format!("{tag}[");
    let suffix = "].tag.tag";
    arguments
        .pairs()
        .iter()
        .filter(|(name, _)| {
            name.starts_with(&prefix) && name.ends_with(suffix) && !name.ends_with("tag-")
        })
        .map(|(_, value)| value.clone())
        .collect()
}

/// The tags one edit removes under `tag`.
fn removals(arguments: &Arguments, tag: &str) -> Vec<String> {
    let key = format!("{tag}[].tag.tag-");
    arguments
        .all(&key)
        .into_iter()
        .flat_map(|joined| joined.split(',').map(str::to_owned).collect::<Vec<_>>())
        .filter(|value| !value.is_empty())
        // Quoted once by the client before the query string quoted it again, so
        // a tag holding a comma survives the join it would otherwise be split
        // by (`plexapi/mixins/edit.py:333`).
        .map(|value| {
            percent_encoding::percent_decode_str(&value)
                .decode_utf8_lossy()
                .into_owned()
        })
        .collect()
}

/// Whether an edit names a field at all — `titleSort`, `title`, a tag.
///
/// A `PUT` naming nothing writes nothing, and a fake that reported it as a
/// write would let a caller believe a request with no arguments saved something.
fn writes_anything(arguments: &Arguments) -> bool {
    arguments.pairs().iter().any(|(name, _)| {
        matches!(name.rsplit('.').next(), Some("value" | "locked"))
            || name.contains("].tag.tag")
            || name == "collectionMode"
            || name == "collectionSort"
    })
}

/// Applies one edit to one item. Returns whether anything was written.
pub(crate) fn apply_to_item(item: &mut FakeItem, arguments: &Arguments) -> bool {
    if !writes_anything(arguments) {
        return false;
    }
    if let Some(title) = arguments.first("title.value") {
        title.clone_into(&mut item.title);
    }
    // Value and lock are independent, and both are written whenever they are
    // sent — including to `0`. A fake that only ever set the lock would make a
    // restore that forgot to clear it look correct (`I-REV-3`, §15.6).
    if let Some(sort_title) = arguments.first("titleSort.value") {
        item.sort_title = Some(sort_title.to_owned());
    }
    if let Some(locked) = arguments.first("titleSort.locked") {
        item.sort_title_locked = locked != "0";
    }
    apply_tags(&mut item.labels, &mut item.labels_locked, arguments);
    true
}

/// Applies one edit to one collection. Returns whether anything was written.
pub(crate) fn apply_to_collection(collection: &mut FakeCollection, arguments: &Arguments) -> bool {
    if !writes_anything(arguments) {
        return false;
    }
    if let Some(title) = arguments.first("title.value") {
        title.clone_into(&mut collection.title);
    }
    if let Some(sort_title) = arguments.first("titleSort.value") {
        collection.sort_title = Some(sort_title.to_owned());
    }
    if let Some(locked) = arguments.first("titleSort.locked") {
        collection.sort_title_locked = locked != "0";
    }
    if let Some(summary) = arguments.first("summary.value") {
        collection.summary = Some(summary.to_owned());
    }
    // Dropped on the floor before, so a collection switched to custom order
    // reported the order it had always reported and every item move under it
    // meant nothing.
    if let Some(mode) = arguments
        .first("collectionMode")
        .and_then(|v| v.parse().ok())
    {
        collection.mode = mode;
    }
    if let Some(sort) = arguments
        .first("collectionSort")
        .and_then(|v| v.parse().ok())
    {
        collection.sort = sort;
    }
    true
}

/// Adds, removes, and locks one tag field.
fn apply_tags(tags: &mut Vec<String>, locked: &mut bool, arguments: &Arguments) {
    for removed in removals(arguments, "label") {
        tags.retain(|tag| tag != &removed);
    }
    for added in additions(arguments, "label") {
        if !tags.contains(&added) {
            tags.push(added);
        }
    }
    // The lock accompanies every tag edit a real client sends
    // (`plexapi/mixins/edit.py:328-330`), and it defaults to *locked* there. A
    // field left locked is the `I-REV-3` failure on the one field the operator
    // touches daily, so the fake has to be able to show it.
    if let Some(value) = arguments.first("label.locked") {
        *locked = value != "0";
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{library::World, scenario::Scenario};

    fn item() -> FakeItem {
        World::build(&Scenario::behaving(1))
            .libraries
            .swap_remove(0)
            .items
            .swap_remove(0)
    }

    fn collection() -> FakeCollection {
        World::build(&Scenario::behaving(1))
            .libraries
            .swap_remove(0)
            .collections
            .swap_remove(0)
    }

    #[test]
    fn an_edit_names_every_id_it_carries() {
        assert_eq!(
            targets(&Arguments::parse(Some("id=1001,1002,1003&type=1"))),
            ["1001", "1002", "1003"]
        );
        assert!(targets(&Arguments::parse(Some("type=1"))).is_empty());
        assert_eq!(libtype(&Arguments::parse(Some("type=18"))), Some(18));
    }

    #[test]
    fn an_items_sort_title_round_trips_in_all_three_of_its_properties() {
        // Nothing could write either field before: every non-label edit went to
        // a collection, so an item's sort title was unreachable.
        let mut item = item();
        assert!(apply_to_item(
            &mut item,
            &Arguments::parse(Some("titleSort.value=!001 Alien&titleSort.locked=1"))
        ));
        assert_eq!(item.sort_title.as_deref(), Some("!001 Alien"));
        assert!(item.sort_title_locked);

        assert!(apply_to_item(
            &mut item,
            &Arguments::parse(Some("titleSort.value=Alien&titleSort.locked=0"))
        ));
        assert!(!item.sort_title_locked, "unlocking is a write too");
    }

    #[test]
    fn a_two_label_removal_removes_two_labels() {
        // One comma-joined value under one key, which is how a real client
        // sends it. Read as a repeated key, this removed one and reported
        // success.
        let mut item = item();
        item.labels = vec!["old".to_owned(), "older".to_owned(), "kept".to_owned()];
        apply_to_item(
            &mut item,
            &Arguments::parse(Some("label[].tag.tag-=old,older&label.locked=0")),
        );
        assert_eq!(item.labels, ["kept"]);
    }

    #[test]
    fn a_label_holding_a_comma_survives_the_join_it_would_be_split_by() {
        // Quoted twice on the way out: once by the client into the joined list
        // and once by the query string. One decode happens before this reads
        // it, so the comma is still `%2C` here and the split is unambiguous.
        let mut item = item();
        item.labels = vec!["a,b".to_owned(), "c".to_owned()];
        apply_to_item(
            &mut item,
            &Arguments::parse(Some("label[].tag.tag-=a%252Cb&label.locked=0")),
        );
        assert_eq!(item.labels, ["c"]);
    }

    #[test]
    fn a_tag_edit_writes_the_lock_that_accompanies_it() {
        // A real client locks the field by default. A fake that ignored the
        // argument could not show the `I-REV-3` failure on the one field the
        // operator edits daily.
        let mut item = item();
        apply_to_item(
            &mut item,
            &Arguments::parse(Some("label[0].tag.tag=afisharr&label.locked=1")),
        );
        assert_eq!(item.labels, ["afisharr"]);
        assert!(item.labels_locked);

        apply_to_item(
            &mut item,
            &Arguments::parse(Some("label[0].tag.tag=afisharr&label.locked=0")),
        );
        assert!(!item.labels_locked);
    }

    #[test]
    fn a_collection_edit_applies_the_three_fields_that_used_to_be_dropped() {
        let mut collection = collection();
        assert!(apply_to_collection(
            &mut collection,
            &Arguments::parse(Some(
                "summary.value=A+few+films&collectionMode=1&collectionSort=2"
            ))
        ));
        assert_eq!(collection.summary.as_deref(), Some("A few films"));
        assert_eq!(collection.mode, 1);
        assert_eq!(collection.sort, 2);
    }

    #[test]
    fn an_edit_that_names_no_field_writes_nothing() {
        let mut collection = collection();
        assert!(!apply_to_collection(
            &mut collection,
            &Arguments::parse(Some("id=15001&type=18"))
        ));
    }
}
