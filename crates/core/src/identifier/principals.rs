// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The three principal identifiers migration `0001` seeds.

/// The `Everyone` principal — every viewer of the server.
pub const EVERYONE: &str = "00000000000000000000000001";

/// The `Owner` principal — the server owner's own account.
pub const OWNER: &str = "00000000000000000000000002";

/// The `SharedAll` principal — every account the library is shared with.
pub const SHARED_ALL: &str = "00000000000000000000000003";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identifier::Id;

    #[test]
    fn the_seeded_identifiers_are_valid_ulids() {
        for seeded in [EVERYONE, OWNER, SHARED_ALL] {
            assert!(Id::parse(seeded).is_ok(), "{seeded} must parse as a ULID");
        }
    }

    #[test]
    fn the_seeded_identifiers_are_distinct() {
        assert_ne!(EVERYONE, OWNER);
        assert_ne!(OWNER, SHARED_ALL);
        assert_ne!(EVERYONE, SHARED_ALL);
    }
}
