// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The bootstrap token: sixty-two bits that prove console access.

use std::{fmt, sync::RwLock};

use subtle::ConstantTimeEq;

use crate::{
    entropy,
    time::{Clock, Timestamp},
};

/// Fifteen minutes (PRD §19.6.1).
///
/// Deliberately longer than the ten-minute claim: the claim must expire while
/// the token that created it is still usable, so an operator whose browser
/// died at step 3 waits out the claim and re-enters a token they already have.
pub const TOKEN_LIFETIME_MILLIS: i64 = 15 * 60 * 1000;

/// The alphabet, the segment length, and the segment count, in one place.
pub const TOKEN_SHAPE: Shape = Shape {
    alphabet: b"abcdefghijklmnopqrstuvwxyz0123456789",
    segment_length: 4,
    segments: 3,
};

/// The shape of a printed token.
#[derive(Debug, Clone, Copy)]
pub struct Shape {
    /// The 36 characters a token is drawn from.
    pub alphabet: &'static [u8; 36],
    /// Characters per hyphen-separated segment.
    pub segment_length: usize,
    /// How many segments.
    pub segments: usize,
}

impl Shape {
    /// The rendered length, counting the hyphens between segments.
    #[must_use]
    pub const fn rendered_length(&self) -> usize {
        self.segments * self.segment_length + (self.segments - 1)
    }

    /// The entropy the shape claims, in bits.
    #[must_use]
    pub fn entropy_bits(&self) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let characters = (self.segments * self.segment_length) as f64;
        characters * 36f64.log2()
    }
}

/// The largest multiple of 36 that fits in a byte.
///
/// Bytes at or above this are discarded and redrawn rather than reduced modulo
/// 36. Without the rejection, `a` through `d` appear more often than the rest
/// and the 62-bit claim is one the generator does not honour (PRD §19.6.1).
const REJECTION_CEILING: u8 = 252;

/// A live bootstrap token.
///
/// Neither `Debug` nor `Display` nor `Clone`: the value must reach the console
/// banner and nothing else — no table, no response body, no line of
/// `logs/afisharr.log` (`I-SEC-8`). The only ways out are
/// [`BootstrapToken::rendered`], which the banner calls once, and a
/// constant-time comparison.
pub struct BootstrapToken {
    value: String,
    expires_at: Timestamp,
}

impl BootstrapToken {
    /// Draws a token from the OS CSPRNG with rejection sampling.
    #[must_use]
    pub fn mint(clock: &dyn Clock) -> Self {
        let mut value = String::with_capacity(TOKEN_SHAPE.rendered_length());
        for segment in 0..TOKEN_SHAPE.segments {
            if segment > 0 {
                value.push('-');
            }
            for _ in 0..TOKEN_SHAPE.segment_length {
                value.push(char::from(TOKEN_SHAPE.alphabet[draw_index()]));
            }
        }
        Self {
            value,
            expires_at: clock.now().plus_millis(TOKEN_LIFETIME_MILLIS),
        }
    }

    /// The token as the banner prints it. The one place the value escapes.
    #[must_use]
    pub fn rendered(&self) -> &str {
        &self.value
    }

    /// When this token stops being accepted.
    #[must_use]
    pub const fn expires_at(&self) -> Timestamp {
        self.expires_at
    }

    /// Whether `candidate` is this token, at `now`.
    ///
    /// Ordered as PRD §19.6.1 fixes it: unexpired, then length, then a
    /// constant-time comparison. Length bounds the work a caller can force,
    /// and the comparison is constant-time because a byte-at-a-time compare
    /// leaks the position of the first mismatch.
    #[must_use]
    pub fn matches(&self, candidate: &str, now: Timestamp) -> bool {
        if now >= self.expires_at {
            return false;
        }
        if candidate.len() != self.value.len() {
            return false;
        }
        self.value.as_bytes().ct_eq(candidate.as_bytes()).into()
    }
}

/// One index into the alphabet, drawn without modulo bias.
fn draw_index() -> usize {
    loop {
        let [byte] = entropy::bytes::<1>();
        if byte < REJECTION_CEILING {
            return usize::from(byte) % 36;
        }
    }
}

/// The one live token, held in process memory for the life of the process.
///
/// Reads dominate — every claim attempt reads, and only a start or a completed
/// setup writes — so this is an `RwLock` rather than a `Mutex`. The guard is
/// never held across an `.await`: every method takes it, decides, and drops it
/// inside a synchronous body.
#[derive(Default)]
pub struct TokenStore {
    live: RwLock<Option<BootstrapToken>>,
}

impl fmt::Debug for TokenStore {
    /// Reports whether a token is held, never which one.
    ///
    /// `BootstrapToken` is deliberately not `Debug`, and a derived
    /// implementation here would reintroduce the value through the field.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenStore")
            .field("holds_a_token", &self.read(|token| token.is_some()))
            .finish()
    }
}

impl TokenStore {
    /// An empty store, for an instance whose setup is already complete.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Mints a token, replacing any predecessor, and returns it for the banner.
    ///
    /// Only one token is live at a time. A restart therefore invalidates the
    /// previous one, which is the second of the three events that end a
    /// token's life (PRD §19.6.1).
    pub fn mint(&self, clock: &dyn Clock) -> String {
        let token = BootstrapToken::mint(clock);
        let rendered = token.rendered().to_owned();
        self.replace(Some(token));
        rendered
    }

    /// Whether a token is live and unexpired at `now`.
    #[must_use]
    pub fn is_live(&self, now: Timestamp) -> bool {
        self.read(|token| token.is_some_and(|token| now < token.expires_at))
    }

    /// Validates `candidate` without consuming the token.
    ///
    /// Check-and-keep, not consume: leaving the token live is what makes a lost
    /// claim cookie recoverable inside the fifteen-minute window. Consuming it
    /// would strand the operator on their own console (PRD §19.6.1).
    #[must_use]
    pub fn accepts(&self, candidate: &str, now: Timestamp) -> bool {
        self.read(|token| token.is_some_and(|token| token.matches(candidate, now)))
    }

    /// Drops the live token. Completing setup calls this.
    pub fn clear(&self) {
        self.replace(None);
    }

    fn read<T>(&self, f: impl FnOnce(Option<&BootstrapToken>) -> T) -> T {
        // A poisoned lock means a panic happened while a token was being
        // swapped. Treating the token as absent is the safe reading: it
        // refuses claims rather than accepting one against a half-written
        // value (P2 — the safe direction changes least).
        match self.live.read() {
            Ok(guard) => f(guard.as_ref()),
            Err(poisoned) => f(poisoned.get_ref().as_ref()),
        }
    }

    fn replace(&self, token: Option<BootstrapToken>) {
        match self.live.write() {
            Ok(mut guard) => *guard = token,
            Err(poisoned) => *poisoned.into_inner() = token,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::FixedClock;

    fn clock() -> FixedClock {
        FixedClock::at(Timestamp::from_millis(1_700_000_000_000))
    }

    #[test]
    fn a_token_is_three_four_character_segments_from_the_declared_alphabet() {
        let token = BootstrapToken::mint(&clock());
        let rendered = token.rendered();
        assert_eq!(rendered.len(), TOKEN_SHAPE.rendered_length());
        let segments: Vec<&str> = rendered.split('-').collect();
        assert_eq!(segments.len(), TOKEN_SHAPE.segments);
        for segment in segments {
            assert_eq!(segment.len(), TOKEN_SHAPE.segment_length);
            assert!(
                segment
                    .bytes()
                    .all(|byte| TOKEN_SHAPE.alphabet.contains(&byte)),
                "{segment}"
            );
        }
    }

    #[test]
    fn the_shape_carries_the_sixty_two_bits_the_prd_claims() {
        assert!(
            (62.0..63.0).contains(&TOKEN_SHAPE.entropy_bits()),
            "{}",
            TOKEN_SHAPE.entropy_bits()
        );
    }

    #[test]
    fn the_rejection_ceiling_is_a_whole_multiple_of_the_alphabet() {
        assert_eq!(
            usize::from(REJECTION_CEILING) % TOKEN_SHAPE.alphabet.len(),
            0
        );
    }

    #[test]
    fn every_character_of_the_alphabet_is_reachable() {
        // A biased or truncated draw shows up here: 36 symbols over enough
        // draws must all appear, and a modulo-reduced byte would still pass
        // this, which is why the ceiling has its own assertion above.
        let clock = clock();
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..400 {
            seen.extend(BootstrapToken::mint(&clock).rendered().bytes());
        }
        seen.remove(&b'-');
        assert_eq!(seen.len(), TOKEN_SHAPE.alphabet.len(), "{seen:?}");
    }

    #[test]
    fn a_token_matches_itself_and_nothing_else() {
        let clock = clock();
        let token = BootstrapToken::mint(&clock);
        let value = token.rendered().to_owned();
        assert!(token.matches(&value, clock.now()));
        assert!(!token.matches("aaaa-aaaa-aaaa", clock.now()));
    }

    #[test]
    fn a_token_stops_matching_exactly_at_fifteen_minutes() {
        let clock = clock();
        let token = BootstrapToken::mint(&clock);
        let value = token.rendered().to_owned();
        assert!(token.matches(&value, clock.now().plus_millis(TOKEN_LIFETIME_MILLIS - 1)));
        assert!(!token.matches(&value, clock.now().plus_millis(TOKEN_LIFETIME_MILLIS)));
    }

    #[test]
    fn wrong_expired_malformed_and_empty_are_all_simply_false() {
        let clock = clock();
        let token = BootstrapToken::mint(&clock);
        let expired = clock.now().plus_millis(TOKEN_LIFETIME_MILLIS);
        assert!(!token.matches("zzzz-zzzz-zzzz", clock.now()));
        assert!(!token.matches(token.rendered(), expired));
        assert!(!token.matches("not a token at all", clock.now()));
        assert!(!token.matches("", clock.now()));
    }

    #[test]
    fn minting_replaces_the_predecessor() {
        let clock = clock();
        let store = TokenStore::empty();
        let first = store.mint(&clock);
        let second = store.mint(&clock);
        assert_ne!(first, second);
        assert!(!store.accepts(&first, clock.now()));
        assert!(store.accepts(&second, clock.now()));
    }

    #[test]
    fn an_empty_store_accepts_nothing() {
        let clock = clock();
        let store = TokenStore::empty();
        assert!(!store.is_live(clock.now()));
        assert!(!store.accepts("aaaa-aaaa-aaaa", clock.now()));
        assert!(!store.accepts("", clock.now()));
    }

    #[test]
    fn validation_keeps_the_token_live_rather_than_consuming_it() {
        let clock = clock();
        let store = TokenStore::empty();
        let value = store.mint(&clock);
        assert!(store.accepts(&value, clock.now()));
        assert!(store.accepts(&value, clock.now()), "a second use must work");
    }

    #[test]
    fn clearing_the_store_ends_the_tokens_life() {
        let clock = clock();
        let store = TokenStore::empty();
        let value = store.mint(&clock);
        store.clear();
        assert!(!store.is_live(clock.now()));
        assert!(!store.accepts(&value, clock.now()));
    }
}
