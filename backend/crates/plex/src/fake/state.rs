// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The world the fake serves, and the ways it can be made to misbehave.

/// One item in the fake's library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeItem {
    /// The key Plex currently answers with. Churns (`I-ID-1`).
    pub rating_key: String,
    /// The identity that survives churn — what the item actually is.
    pub guid: String,
    /// `movie`, `show`, and so on.
    pub kind: String,
    /// The title.
    pub title: String,
    /// The sort title's value. `None` is the attribute being absent, which is
    /// a different fact from it being equal to the title (§15.6).
    pub sort_title: Option<String>,
    /// Whether Plex's metadata lock is set on the sort title.
    pub sort_title_locked: bool,
    /// The release year.
    pub year: Option<i32>,
    /// The poster reference, in whatever format this scenario chose.
    pub thumb: String,
    /// Whether Plex has finished indexing it. `false` is the partial scan
    /// state `I-EVID-*` is written against.
    pub indexed: bool,
    /// Whether the item has a media file this scenario reports.
    pub has_media: bool,
    /// The labels on it.
    pub labels: Vec<String>,
}

/// One collection in the fake's library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeCollection {
    /// Plex's key for it.
    pub rating_key: String,
    /// The title.
    pub title: String,
    /// The sort title's value, absent until something writes one.
    pub sort_title: Option<String>,
    /// Whether the sort title is locked.
    pub sort_title_locked: bool,
    /// The rating keys it holds, in order.
    pub items: Vec<String>,
}

/// One row in the fake's ordering space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeHub {
    /// Plex's identifier for the row.
    pub identifier: String,
    /// The row's title.
    pub title: String,
    /// The collection behind it, or `None` for one of Plex's own rows.
    ///
    /// A native row cannot be unpromoted, which is what makes it an anchor
    /// rather than a participant (§15.1).
    pub rating_key: Option<String>,
    /// Visible on the owner's home screen.
    pub own_home: bool,
    /// Visible on shared users' home screens.
    pub shared_home: bool,
    /// Visible on the library's recommended row.
    pub recommended: bool,
}

/// One library in the fake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeLibrary {
    /// The section key.
    pub key: String,
    /// The section uuid, stable across a key change.
    pub uuid: String,
    /// `movie`, `show`, `artist`, `photo`.
    pub kind: String,
    /// The title.
    pub title: String,
    /// The items in it, in the order they are listed.
    pub items: Vec<FakeItem>,
    /// The collections in it.
    pub collections: Vec<FakeCollection>,
    /// The ordering space, in order.
    pub hubs: Vec<FakeHub>,
    /// How many moves this surface has left before they silently no-op.
    ///
    /// The precision budget §15.3 describes, counted down rather than modelled
    /// as midpoint arithmetic: the observable behaviour is what a test needs,
    /// and the arithmetic is Plex's business.
    pub moves_left: u32,
}

impl FakeLibrary {
    /// Moves a hub after another, honouring the precision budget.
    ///
    /// Returns `true` when the order actually changed. Either way the caller
    /// answers 200 — that is the whole misbehaviour: past the budget Plex
    /// reports success and leaves the order alone, and only a verification read
    /// can tell the difference (`I-CONV-*`, §15.3).
    pub fn move_hub(&mut self, identifier: &str, after: Option<&str>) -> bool {
        let Some(from) = self
            .hubs
            .iter()
            .position(|hub| hub.identifier == identifier)
        else {
            return false;
        };
        // The predecessor is resolved against the space as it stands, before the
        // removal, and a predecessor that is not a row in it is a refusal rather
        // than an append. Looked up afterwards, `map_or(len)` read "not found"
        // as "put it last" — so a row naming *itself* as its predecessor, or one
        // naming a row that has since gone, teleported to the tail, spent a move
        // from the budget, and answered 200. A verification read then sees an
        // order nobody asked for, which is a different failure from the silent
        // no-op §15.3 is about and would be mistaken for it.
        let target = match after {
            None => None,
            Some(after) => {
                match self
                    .hubs
                    .iter()
                    .position(|other| other.identifier == after)
                {
                    None => return false,
                    Some(index) if index == from => return false,
                    Some(index) => Some(index),
                }
            }
        };
        if self.moves_left == 0 {
            return false;
        }
        let hub = self.hubs.remove(from);
        let to = match target {
            None => 0,
            // The predecessor's index shifts down by one when the row being
            // moved sat in front of it, and does not when it sat behind.
            Some(index) if index > from => index,
            Some(index) => index + 1,
        };
        self.hubs.insert(to, hub);
        self.moves_left -= 1;
        true
    }

    /// Moves an item inside a collection, honouring the same budget.
    pub fn move_collection_item(
        &mut self,
        collection: &str,
        item: &str,
        after: Option<&str>,
    ) -> bool {
        let budget = self.moves_left;
        let Some(collection) = self
            .collections
            .iter_mut()
            .find(|candidate| candidate.rating_key == collection)
        else {
            return false;
        };
        let Some(from) = collection.items.iter().position(|key| key == item) else {
            return false;
        };
        // Resolved before the removal, and refused when it names nothing in the
        // collection or names the item being moved — see `move_hub` for what
        // the append-on-miss it replaces did to a verification read.
        let target = match after {
            None => None,
            Some(after) => match collection.items.iter().position(|other| other == after) {
                None => return false,
                Some(index) if index == from => return false,
                Some(index) => Some(index),
            },
        };
        if budget == 0 {
            return false;
        }
        let key = collection.items.remove(from);
        let to = match target {
            None => 0,
            Some(index) if index > from => index,
            Some(index) => index + 1,
        };
        collection.items.insert(to, key);
        self.moves_left -= 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hub(identifier: &str) -> FakeHub {
        FakeHub {
            identifier: identifier.to_owned(),
            title: identifier.to_owned(),
            rating_key: None,
            own_home: true,
            shared_home: false,
            recommended: false,
        }
    }

    fn library(moves_left: u32) -> FakeLibrary {
        FakeLibrary {
            key: "1".to_owned(),
            uuid: "uuid-1".to_owned(),
            kind: "movie".to_owned(),
            title: "Movies".to_owned(),
            items: Vec::new(),
            collections: vec![FakeCollection {
                rating_key: "5001".to_owned(),
                title: "Best".to_owned(),
                sort_title: None,
                sort_title_locked: false,
                items: vec!["1".to_owned(), "2".to_owned(), "3".to_owned()],
            }],
            hubs: vec![hub("a"), hub("b"), hub("c")],
            moves_left,
        }
    }

    fn order(library: &FakeLibrary) -> Vec<&str> {
        library
            .hubs
            .iter()
            .map(|hub| hub.identifier.as_str())
            .collect()
    }

    #[test]
    fn a_move_within_budget_changes_the_order() {
        let mut library = library(10);
        assert!(library.move_hub("c", Some("a")));
        assert_eq!(order(&library), ["a", "c", "b"]);
        assert_eq!(library.moves_left, 9);
    }

    #[test]
    fn a_move_to_the_front_has_no_predecessor() {
        let mut library = library(10);
        assert!(library.move_hub("c", None));
        assert_eq!(order(&library), ["c", "a", "b"]);
    }

    #[test]
    fn a_move_past_the_budget_reports_nothing_and_changes_nothing() {
        // The silent no-op: the endpoint still answers 200, and only reading
        // the order back shows the item never moved (§15.3).
        let mut library = library(1);
        assert!(library.move_hub("c", Some("a")));
        assert!(!library.move_hub("b", Some("c")));
        assert_eq!(order(&library), ["a", "c", "b"]);
        assert_eq!(library.moves_left, 0);
    }

    #[test]
    fn moving_an_item_that_is_not_there_spends_no_budget() {
        let mut library = library(3);
        assert!(!library.move_hub("nowhere", None));
        assert_eq!(library.moves_left, 3);
    }

    #[test]
    fn a_predecessor_that_is_not_in_the_space_moves_nothing_and_spends_nothing() {
        // Resolved after the removal it was "not found", and not-found was read
        // as "append": the row teleported to the tail, the budget paid for it,
        // and the call answered 200. A verification read then sees a wrong
        // order rather than an unchanged one, which is a different failure from
        // the silent no-op §15.3 describes and would be mistaken for it.
        let mut library = library(3);
        assert!(!library.move_hub("a", Some("nowhere")));
        assert_eq!(order(&library), ["a", "b", "c"]);
        assert_eq!(library.moves_left, 3);
    }

    #[test]
    fn a_row_cannot_be_moved_after_itself() {
        // `HubMove::After` does not forbid it, and the old lookup found nothing
        // once the row had been removed — so asking for no change moved the row
        // to the end of the space.
        let mut library = library(3);
        assert!(!library.move_hub("a", Some("a")));
        assert_eq!(order(&library), ["a", "b", "c"]);
        assert_eq!(library.moves_left, 3);
    }

    #[test]
    fn a_move_backwards_lands_directly_after_its_predecessor() {
        // The index arithmetic the pre-removal lookup makes necessary: `b` sits
        // behind `a`, so `a`'s position is unchanged by the removal.
        let mut library = library(10);
        assert!(library.move_hub("a", Some("b")));
        assert_eq!(order(&library), ["b", "a", "c"]);
    }

    #[test]
    fn a_collection_item_move_after_a_key_that_is_not_in_it_changes_nothing() {
        let mut library = library(3);
        assert!(!library.move_collection_item("5001", "3", Some("nowhere")));
        assert_eq!(library.collections[0].items, ["1", "2", "3"]);
        assert_eq!(library.moves_left, 3);
    }

    #[test]
    fn a_collection_item_move_uses_the_same_budget_as_a_hub_move() {
        // One ordering space, one budget: a test that spent hub moves and then
        // found collection moves still working would be testing a fake with
        // more precision than Plex has.
        let mut library = library(1);
        assert!(library.move_collection_item("5001", "3", None));
        assert_eq!(library.collections[0].items, ["3", "1", "2"]);
        assert!(!library.move_hub("c", Some("a")));
    }
}
