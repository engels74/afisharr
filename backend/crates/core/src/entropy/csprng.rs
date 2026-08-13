// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Bytes from the operating system's cryptographically secure generator.

/// `N` bytes from the OS CSPRNG.
///
/// ```
/// use afisharr_core::entropy;
///
/// let a: [u8; 16] = entropy::bytes();
/// let b: [u8; 16] = entropy::bytes();
/// assert_ne!(a, b);
/// ```
///
/// # Panics
/// Panics when the operating system's entropy source is unavailable. On every
/// platform PRD §21.5 supports that means the process cannot continue, and
/// returning a `Result` here would push a decision nobody can make sensibly
/// out to five call sites — a session identifier drawn from a degraded source
/// is worse than no session at all.
#[must_use]
pub fn bytes<const N: usize>() -> [u8; N] {
    let mut buffer = [0u8; N];
    getrandom::fill(&mut buffer).expect("the OS CSPRNG is available on every supported platform");
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_requested_length_is_what_comes_back() {
        assert_eq!(bytes::<1>().len(), 1);
        assert_eq!(bytes::<64>().len(), 64);
    }

    #[test]
    fn two_draws_of_thirty_two_bytes_differ() {
        assert_ne!(bytes::<32>(), bytes::<32>());
    }

    #[test]
    fn a_zero_length_draw_is_the_empty_array() {
        assert_eq!(bytes::<0>(), [0u8; 0]);
    }
}
