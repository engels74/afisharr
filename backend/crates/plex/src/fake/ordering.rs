// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Moving a row, and the precision budget that makes a move stop happening.
//!
//! The budget is counted down rather than modelled as midpoint arithmetic: the
//! observable behaviour is what a test needs, and the arithmetic is Plex's
//! business. Past it every move still answers 200 and changes nothing, which
//! is why every applied plan is verified by reading the order back (§15.3).
//!
//! **One budget per sequence.** The hub space has its own and so does every
//! collection, because they are separate sequences on a real server: a single
//! counter made a per-collection budget untestable, and left an
//! escalation-ladder test unable to say which sequence had run out.

use crate::fake::state::FakeLibrary;

/// Where a row is being moved to, resolved against the sequence it is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Destination {
    /// To the head of the sequence.
    Front,
    /// Immediately after the row at this index, as the sequence stands now.
    After(usize),
    /// Nowhere: the predecessor is not a row in this sequence, or it is the
    /// row being moved.
    Nowhere,
}

impl Destination {
    /// Resolves a predecessor against the sequence, *before* the removal.
    ///
    /// Looked up afterwards, "not found" read as "put it last" — so a row
    /// naming itself as its predecessor, or one naming a row that has since
    /// gone, teleported to the tail, spent a move from the budget, and answered
    /// 200. A verification read then sees an order nobody asked for, which is a
    /// different failure from the silent no-op §15.3 is about and would be
    /// mistaken for it.
    fn resolve(sequence: &[String], from: usize, after: Option<&str>) -> Self {
        let Some(after) = after else {
            return Self::Front;
        };
        match sequence.iter().position(|other| other == after) {
            Some(index) if index != from => Self::After(index),
            _ => Self::Nowhere,
        }
    }

    /// Where the moved row lands, once it has been taken out.
    ///
    /// The predecessor's index shifts down by one when the row being moved sat
    /// in front of it, and does not when it sat behind.
    const fn landing(self, from: usize) -> usize {
        match self {
            Self::After(index) if index > from => index,
            Self::After(index) => index + 1,
            _ => 0,
        }
    }
}

impl FakeLibrary {
    /// Moves a hub after another, honouring the hub space's budget.
    ///
    /// Returns `true` when the order actually changed. Either way the caller
    /// answers 200 — that is the whole misbehaviour (`I-CONV-*`, §15.3).
    pub fn move_hub(&mut self, identifier: &str, after: Option<&str>) -> bool {
        let order: Vec<String> = self.hubs.iter().map(|hub| hub.identifier.clone()).collect();
        let Some(from) = order.iter().position(|other| other == identifier) else {
            return false;
        };
        let target = Destination::resolve(&order, from, after);
        if target == Destination::Nowhere || self.hub_moves_left == 0 {
            return false;
        }
        let hub = self.hubs.remove(from);
        self.hubs.insert(target.landing(from), hub);
        self.hub_moves_left -= 1;
        true
    }

    /// Moves an item inside a collection, honouring that collection's budget.
    pub fn move_collection_item(
        &mut self,
        collection: &str,
        item: &str,
        after: Option<&str>,
    ) -> bool {
        let Some(collection) = self.collection(collection) else {
            return false;
        };
        let Some(from) = collection.items.iter().position(|key| key == item) else {
            return false;
        };
        let target = Destination::resolve(&collection.items, from, after);
        if target == Destination::Nowhere || collection.moves_left == 0 {
            return false;
        }
        let key = collection.items.remove(from);
        collection.items.insert(target.landing(from), key);
        collection.moves_left -= 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::fake::{
        library,
        scenario::Scenario,
        state::{FakeCollection, FakeHub, FakeLibrary},
    };

    fn hub(identifier: &str) -> FakeHub {
        FakeHub {
            identifier: identifier.to_owned(),
            title: identifier.to_owned(),
            rating_key: None,
            deletable: false,
            own_home: true,
            shared_home: false,
            recommended: false,
        }
    }

    fn collection(rating_key: &str, moves_left: u32) -> FakeCollection {
        FakeCollection {
            rating_key: rating_key.to_owned(),
            title: "Best".to_owned(),
            sort_title: None,
            sort_title_locked: false,
            summary: None,
            subtype: "movie".to_owned(),
            mode: -1,
            sort: 0,
            smart: false,
            items: vec!["1".to_owned(), "2".to_owned(), "3".to_owned()],
            moves_left,
        }
    }

    fn library(moves_left: u32) -> FakeLibrary {
        FakeLibrary {
            key: "1".to_owned(),
            uuid: "uuid-1".to_owned(),
            kind: "movie".to_owned(),
            title: "Movies".to_owned(),
            scanner: "Plex Movie".to_owned(),
            locations: vec!["/data/movies".to_owned()],
            items: Vec::new(),
            collections: vec![
                collection("5001", moves_left),
                collection("5002", moves_left),
            ],
            hubs: vec![hub("a"), hub("b"), hub("c")],
            hub_moves_left: moves_left,
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
        assert_eq!(library.hub_moves_left, 9);
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
        assert_eq!(library.hub_moves_left, 0);
    }

    #[test]
    fn moving_an_item_that_is_not_there_spends_no_budget() {
        let mut library = library(3);
        assert!(!library.move_hub("nowhere", None));
        assert_eq!(library.hub_moves_left, 3);
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
        assert_eq!(library.hub_moves_left, 3);
    }

    #[test]
    fn a_row_cannot_be_moved_after_itself() {
        let mut library = library(3);
        assert!(!library.move_hub("a", Some("a")));
        assert_eq!(order(&library), ["a", "b", "c"]);
        assert_eq!(library.hub_moves_left, 3);
    }

    #[test]
    fn a_move_backwards_lands_directly_after_its_predecessor() {
        let mut library = library(10);
        assert!(library.move_hub("a", Some("b")));
        assert_eq!(order(&library), ["b", "a", "c"]);
    }

    #[test]
    fn a_collection_item_move_after_a_key_that_is_not_in_it_changes_nothing() {
        let mut library = library(3);
        assert!(!library.move_collection_item("5001", "3", Some("nowhere")));
        assert_eq!(library.collections[0].items, ["1", "2", "3"]);
        assert_eq!(library.collections[0].moves_left, 3);
    }

    #[test]
    fn one_collections_budget_running_out_leaves_another_collections_alone() {
        // One counter across every sequence made this case unreachable, and an
        // escalation-ladder test could not say which sequence had run out.
        let mut library = library(1);
        assert!(library.move_collection_item("5001", "3", None));
        assert!(!library.move_collection_item("5001", "2", None));
        assert!(
            library.move_collection_item("5002", "3", None),
            "the second collection has its own budget"
        );
    }

    #[test]
    fn the_hub_space_and_a_collection_do_not_spend_each_others_budget() {
        let mut library = library(1);
        assert!(library.move_collection_item("5001", "3", None));
        assert!(
            library.move_hub("c", Some("a")),
            "the hub space's budget is its own"
        );
    }

    #[test]
    fn a_scenarios_move_budget_reaches_every_sequence_it_built() {
        let world = library::World::build(&Scenario::behaving(1).with_move_budget(4));
        assert_eq!(world.libraries[0].hub_moves_left, 4);
        assert_eq!(world.libraries[0].collections[0].moves_left, 4);
    }
}
