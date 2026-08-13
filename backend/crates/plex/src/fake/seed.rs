// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The one seed every fake behaviour is drawn from.

/// A seed, and the stream of values it produces.
///
/// The generator is `SplitMix64`, written out here rather than taken from a
/// crate, and the reason is the fidelity contract itself: D-036 requires
/// byte-identical replay from a seed, and every general-purpose Rust generator
/// documents its output as *not* value-stable across versions. A dependency
/// bump would then change what a scenario means, silently, in a test suite
/// whose whole job is to be reproducible. Fifteen lines of arithmetic with a
/// published constant have no such release note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seed {
    origin: u64,
    state: u64,
}

/// `SplitMix64`'s increment — the odd constant from the reference implementation.
const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

impl Seed {
    /// A stream from `seed`.
    #[must_use]
    pub const fn of(seed: u64) -> Self {
        Self {
            origin: seed,
            state: seed,
        }
    }

    /// The value this stream started from.
    #[must_use]
    pub const fn origin(&self) -> u64 {
        self.origin
    }

    /// The same stream, back at its beginning.
    ///
    /// What makes "the same seed twice" a checkable claim rather than an
    /// intention: a test runs a scenario, rewinds, and runs it again.
    #[must_use]
    pub const fn rewound(&self) -> Self {
        Self::of(self.origin)
    }

    /// The next value.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GAMMA);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// The next value, below `bound`.
    ///
    /// Rejection-sampled rather than reduced modulo `bound`: a modulo skews the
    /// low values, and a fake whose "one item in five churns" is really one in
    /// 4.7 is a fake nobody can reason about from the seed.
    pub fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return 0;
        }
        let zone = u64::MAX - (u64::MAX % bound);
        loop {
            let drawn = self.next_u64();
            if drawn < zone {
                return drawn % bound;
            }
        }
    }

    /// Whether the next draw falls inside a one-in-`odds` chance.
    ///
    /// Consumes exactly one value whatever `odds` is, `0` included, and that
    /// is the point rather than an implementation detail: the fake draws every
    /// decision for every item whether or not the scenario asked for that
    /// behaviour, so two scenarios sharing a seed agree about the items neither
    /// of them changed. A version that skipped the draw when a behaviour was
    /// off moved the stream's position, and turning on partial scans silently
    /// re-rolled every sort title.
    ///
    /// Reduced modulo rather than rejection-sampled, unlike [`Seed::below`].
    /// The bias is `odds / 2^64` — unmeasurable at any odds a scenario would
    /// state — and rejection sampling would consume a variable number of
    /// values, which is the one property this has to keep.
    pub fn one_in(&mut self, odds: u64) -> bool {
        let drawn = self.next_u64();
        // The guard is not redundant: `u64::is_multiple_of(0)` is `self == 0`,
        // so odds of zero would fire on the one draw in 2^64 that lands there.
        // Odds of zero mean the behaviour was never asked for, and "never"
        // has to mean never rather than almost never.
        odds > 0 && drawn.is_multiple_of(odds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_produces_the_same_stream() {
        let mut first = Seed::of(42);
        let mut second = Seed::of(42);
        let drawn: Vec<u64> = (0..16).map(|_| first.next_u64()).collect();
        let again: Vec<u64> = (0..16).map(|_| second.next_u64()).collect();
        assert_eq!(drawn, again);
    }

    #[test]
    fn a_different_seed_produces_a_different_stream() {
        assert_ne!(Seed::of(42).next_u64(), Seed::of(43).next_u64());
    }

    #[test]
    fn rewinding_returns_the_stream_to_its_beginning() {
        let mut stream = Seed::of(7);
        let first = stream.next_u64();
        for _ in 0..100 {
            stream.next_u64();
        }
        assert_eq!(stream.rewound().next_u64(), first);
        assert_eq!(stream.origin(), 7);
    }

    #[test]
    fn a_bounded_draw_stays_inside_its_bound() {
        let mut stream = Seed::of(1);
        for _ in 0..1000 {
            assert!(stream.below(5) < 5);
        }
        assert_eq!(stream.below(0), 0);
    }

    #[test]
    fn a_bounded_draw_reaches_every_value_in_its_range() {
        // A generator that never returns the top value would make a scenario's
        // rarest behaviour unreachable, which is a fake that quietly cannot
        // produce a row of its own fidelity contract.
        let mut stream = Seed::of(99);
        let mut seen = [false; 5];
        for _ in 0..2000 {
            seen[usize::try_from(stream.below(5)).expect("below 5")] = true;
        }
        assert!(seen.iter().all(|hit| *hit), "{seen:?}");
    }

    #[test]
    fn a_one_in_n_chance_lands_near_its_stated_rate() {
        let mut stream = Seed::of(2024);
        let hits = (0..10_000).filter(|_| stream.one_in(10)).count();
        assert!((800..1200).contains(&hits), "{hits} hits in 10000");
        assert!(!Seed::of(1).one_in(0));
    }

    #[test]
    fn a_decision_costs_one_draw_whether_or_not_it_can_land() {
        // The property the fake's cross-scenario determinism rests on. A
        // version that skipped the draw at odds of zero moved the stream, so
        // turning one behaviour on re-rolled every other one.
        let mut asked = Seed::of(7);
        let mut never = Seed::of(7);
        asked.one_in(3);
        never.one_in(0);
        assert_eq!(asked.next_u64(), never.next_u64());
    }
}
