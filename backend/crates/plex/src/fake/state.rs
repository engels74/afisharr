// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The world the fake serves.
//!
//! The data only. What the world *does* when a move is asked for lives in
//! [`crate::fake::ordering`], because the precision budget is a behaviour with
//! its own tests rather than a field with a getter.

/// One item in the fake's library.
//
// The lint's usual remedy — collapse the flags into a state enum — is what this
// type exists to refuse: each flag is an independent fact a scenario sets on
// its own, and §15.6 turns on exactly that independence.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeItem {
    /// The key Plex currently answers with. Churns (`I-ID-1`).
    pub rating_key: String,
    /// The identity that survives churn — what the item actually is.
    pub guid: String,
    /// The external ids Plex reports alongside the primary guid.
    ///
    /// A separate list because they are separate facts: the primary guid is
    /// Plex's own, and these are the provider ids the resolver matches on.
    pub external_guids: Vec<String>,
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
    /// The season or episode number, when the kind has one.
    pub index: Option<i32>,
    /// The parent's key — the show for a season, the season for an episode.
    pub parent_rating_key: Option<String>,
    /// The civil release date, as Plex spells it.
    pub originally_available_at: Option<String>,
    /// The poster reference, in whatever format this scenario chose.
    pub thumb: String,
    /// Whether Plex has finished indexing it. `false` is the partial scan
    /// state `I-EVID-*` is written against.
    pub indexed: bool,
    /// Whether the item has a media file this scenario reports.
    pub has_media: bool,
    /// The genre tags on it, which the filter arguments match against.
    pub genres: Vec<String>,
    /// The labels on it.
    pub labels: Vec<String>,
    /// Whether Plex's metadata lock is set on the label field.
    ///
    /// Written by every tag edit that names it (`plexapi/mixins/edit.py:328`).
    /// A field left locked is the `I-REV-3` failure on the one field the
    /// operator touches daily, so the fake has to be able to show it.
    pub labels_locked: bool,
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
    /// The summary, absent until something writes one.
    pub summary: Option<String>,
    /// The libtype of the items in it, which Plex reports as `subtype`.
    pub subtype: String,
    /// Plex's `collectionMode`. `-1` is the library default.
    pub mode: i32,
    /// Plex's `collectionSort`. `0` on a new collection — release order — and
    /// custom order is `2`, which is a thing something has to switch on
    /// (`plexapi/collection.py:73`).
    pub sort: i32,
    /// Whether Plex maintains it from a filter rather than from a list.
    ///
    /// A smart collection refuses item edits (`plexapi/collection.py:317`),
    /// which is a refusal nothing could reach while the fake had no way to
    /// mark one.
    pub smart: bool,
    /// The labels on it.
    ///
    /// A collection carries them exactly as an item does: the same edit
    /// endpoint writes them, and the `collection` libtype's own filter
    /// vocabulary declares `label` and nothing else
    /// (`plexapi/library.py:890-899`). A collection that held none could not
    /// answer that filter, and a label edit aimed at one wrote nothing and
    /// answered that it had.
    pub labels: Vec<String>,
    /// Whether Plex's metadata lock is set on the label field.
    pub labels_locked: bool,
    /// The rating keys it holds, in order.
    pub items: Vec<String>,
    /// How many moves this collection accepts before they silently no-op.
    ///
    /// Its own budget, not the library's: one counter across every sequence
    /// made a per-collection budget untestable, and an escalation-ladder test
    /// ambiguous about which sequence ran out (§15.3).
    pub moves_left: u32,
}

/// One row in the fake's ordering space.
//
// Four independent facts a real server reports separately: whether the row can
// leave the space, and its three visibility axes. §15.5 exists because the
// three are independent, so an enum over them would be a vocabulary Afisharr
// invented on top of somebody else's.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeHub {
    /// Plex's identifier for the row.
    pub identifier: String,
    /// The row's title.
    pub title: String,
    /// The collection behind it, or `None` for one of Plex's own rows.
    ///
    /// Known to the fake and never sent: `python-plexapi`'s `ManagedHub` reads
    /// no rating key here (`plexapi/library.py:3033-3046`), so this build has
    /// no evidence a real server puts one in the answer. What the answer says
    /// instead is `deletable`.
    pub rating_key: Option<String>,
    /// Whether the row can be removed from the ordering space.
    ///
    /// How a real server says one of its own rows is an anchor
    /// (`plexapi/library.py:3035`), and the fact §15.1 rests on.
    pub deletable: bool,
    /// Visible on the owner's home screen.
    pub own_home: bool,
    /// Visible on shared users' home screens.
    pub shared_home: bool,
    /// Visible on the library's recommended row.
    pub recommended: bool,
}

impl FakeHub {
    /// How a real server spells the home-screen visibility of this row.
    #[must_use]
    pub(crate) const fn home_visibility(&self) -> &'static str {
        match (self.own_home, self.shared_home) {
            (true, true) => "all",
            (true, false) => "admin",
            (false, true) => "shared",
            (false, false) => "none",
        }
    }

    /// How a real server spells the recommended-row visibility of this row.
    #[must_use]
    pub(crate) const fn recommendations_visibility(&self) -> &'static str {
        if self.recommended { "all" } else { "none" }
    }
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
    /// The scanner Plex reports for it.
    pub scanner: String,
    /// The folders the library is built from.
    pub locations: Vec<String>,
    /// The items in it, in the order they are listed.
    pub items: Vec<FakeItem>,
    /// The collections in it.
    pub collections: Vec<FakeCollection>,
    /// The ordering space, in order.
    pub hubs: Vec<FakeHub>,
    /// How many hub moves this library accepts before they silently no-op.
    pub hub_moves_left: u32,
}

impl FakeLibrary {
    /// The collection with this key.
    pub(crate) fn collection(&mut self, rating_key: &str) -> Option<&mut FakeCollection> {
        self.collections
            .iter_mut()
            .find(|candidate| candidate.rating_key == rating_key)
    }

    /// The item with this key.
    pub(crate) fn item(&mut self, rating_key: &str) -> Option<&mut FakeItem> {
        self.items
            .iter_mut()
            .find(|candidate| candidate.rating_key == rating_key)
    }
}
