// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `server://…` URI Plex names a set of items with.

use crate::{libraries::RatingKey, server::MachineIdentifier};

/// The provider path every library URI carries.
const LIBRARY_PROVIDER: &str = "com.plexapp.plugins.library";

/// Builds the `uri=` argument naming `items` on `server`.
///
/// The machine identifier is a parameter rather than something this crate
/// remembers, because it is the one value that decides *which server's* rating
/// keys these are. A URI built against a stale identifier addresses items on a
/// server the operator no longer runs, which is `I-ID-5`'s failure expressed as
/// a collection full of the wrong films.
///
/// Returns `None` for an empty item set: `uri=…/metadata/` with nothing after
/// it is a request to add every item Plex can find, and the caller who meant
/// "add nothing" would have created a collection of the whole library.
#[must_use]
pub fn library_uri(server: &MachineIdentifier, items: &[RatingKey]) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    let keys: Vec<&str> = items.iter().map(RatingKey::as_str).collect();
    Some(format!(
        "server://{server}/{LIBRARY_PROVIDER}/library/metadata/{}",
        keys.join(",")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(values: &[&str]) -> Vec<RatingKey> {
        values.iter().map(|value| RatingKey::new(*value)).collect()
    }

    #[test]
    fn one_item_names_the_server_and_the_key() {
        let uri = library_uri(&MachineIdentifier::new("abc123"), &keys(&["1001"]))
            .expect("one item is a set");
        assert_eq!(
            uri,
            "server://abc123/com.plexapp.plugins.library/library/metadata/1001"
        );
    }

    #[test]
    fn several_items_are_comma_joined_in_the_order_given() {
        // Order is the caller's: Plex adds them in the order the URI lists, and
        // a set that reordered here would be a collection whose initial order
        // nobody chose.
        let uri = library_uri(&MachineIdentifier::new("abc123"), &keys(&["3", "1", "2"]))
            .expect("three items are a set");
        assert!(uri.ends_with("/library/metadata/3,1,2"), "{uri}");
    }

    #[test]
    fn an_empty_set_has_no_uri_at_all() {
        // `.../metadata/` with nothing after it matches everything, so the call
        // that meant "add nothing" would have added the library.
        assert_eq!(library_uri(&MachineIdentifier::new("abc"), &[]), None);
    }
}
