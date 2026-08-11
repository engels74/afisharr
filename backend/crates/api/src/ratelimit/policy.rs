// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The limit table from PRD §21.4.3, as data.

/// One counted class of request.
///
/// The bucket names *what is being protected*, not which route asked. Two
/// routes that reach a provider share one bucket because the thing being
/// protected — the operator's provider quota — is one thing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Bucket {
    /// Failed sign-ins against one account name.
    LoginAccount {
        /// The username that was tried.
        username: String,
    },
    /// Failed sign-ins from one address.
    LoginAddress,
    /// Claim and recovery attempts from one address.
    SetupAttempt,
    /// Calls made with one accepted credential.
    ///
    /// Keyed on the credential and not on the address, and that is the whole
    /// of what it protects. Counted per address, this bucket is spent by
    /// whoever reaches the instance first — and behind a reverse proxy that
    /// `trustProxy` does not name, every caller resolves to the proxy's own
    /// address, so one unauthenticated flood spends the allowance the
    /// operator's own interface needs and holds the whole surface at 429 for
    /// the rest of the window. A credential is something an attacker has to
    /// obtain before they can spend anything counted under it.
    Api {
        /// The session digest or API key identifier the caller presented.
        credential: String,
    },
    /// Calls from one address that carry no accepted credential.
    ///
    /// Sign-in, the Plex pin exchange, and every request whose credential was
    /// refused. Separate from [`Bucket::Api`] so that traffic anybody can send
    /// cannot spend the budget of callers who have proved who they are.
    Anonymous,
    /// Calls that reach a third-party service on the caller's behalf.
    Provider,
}

/// The longest account name a limiter key keeps.
///
/// The bucket is keyed on the name the caller sent, and a caller chooses both
/// its content and its length. Without a bound, every sign-in attempt parks a
/// string of the caller's choosing in a map that lives as long as the process.
/// The bound is far longer than any name this instance will ever store, so two
/// real accounts cannot collide here; a name past it cannot match an account at
/// all, and counting several of those together only makes the limit stricter.
const KEYED_USERNAME_BYTES: usize = 128;

impl Bucket {
    /// The failed-sign-in bucket for `username`.
    ///
    /// The one way this bucket is built, so the bound above is not something a
    /// call site has to remember (P7).
    #[must_use]
    pub fn login_account(username: &str) -> Self {
        let mut end = username.len().min(KEYED_USERNAME_BYTES);
        while end > 0 && !username.is_char_boundary(end) {
            end -= 1;
        }
        Self::LoginAccount {
            username: username[..end].to_owned(),
        }
    }

    /// The API bucket for one accepted credential.
    ///
    /// `credential` is a server-side identifier — a session digest or an API
    /// key's row id — so the set of keys this bucket can ever hold is the set
    /// of credentials this instance issued. A caller cannot mint a new counter
    /// by inventing a value, because a value this instance did not issue never
    /// reaches here (`guard::Authenticated` refuses it first).
    #[must_use]
    pub fn api(credential: &str) -> Self {
        Self::Api {
            credential: credential.to_owned(),
        }
    }

    /// The limit this bucket is counted against.
    #[must_use]
    pub const fn policy(&self) -> Policy {
        match self {
            // Five failures in fifteen minutes, then a lockout that doubles to
            // twenty-four hours. The lockout is per account rather than per
            // address because an attacker who can vary their address cannot
            // vary the account they are trying to get into.
            Self::LoginAccount { .. } => Policy {
                allowance: 5,
                window_millis: 15 * 60 * 1000,
                lockout: Some(Lockout {
                    initial_millis: 15 * 60 * 1000,
                    ceiling_millis: 24 * 60 * 60 * 1000,
                }),
            },
            Self::LoginAddress => Policy {
                allowance: 20,
                window_millis: 15 * 60 * 1000,
                lockout: None,
            },
            Self::SetupAttempt => Policy {
                allowance: 5,
                window_millis: 15 * 60 * 1000,
                lockout: None,
            },
            Self::Api { .. } => Policy {
                allowance: 600,
                window_millis: 60 * 1000,
                lockout: None,
            },
            // Half the authenticated allowance, and it is not a guess at what
            // an attacker needs — it is what the unauthenticated interface
            // itself needs. The Plex pin exchange polls while the operator is
            // away at plex.tv, and the sign-in page reads the session once per
            // load; both fit inside this several times over, and a caller doing
            // neither has no business making three hundred anonymous calls a
            // minute.
            Self::Anonymous => Policy {
                allowance: 300,
                window_millis: 60 * 1000,
                lockout: None,
            },
            // Protects the operator's provider quota, not this instance.
            Self::Provider => Policy {
                allowance: 60,
                window_millis: 60 * 1000,
                lockout: None,
            },
        }
    }

    /// Whether this bucket counts every request or only the failures.
    ///
    /// A login limit that counted successes would lock out the operator who
    /// signs in from four devices; an API limit that counted only failures
    /// would not be a rate limit at all.
    #[must_use]
    pub const fn counts_failures_only(&self) -> bool {
        matches!(self, Self::LoginAccount { .. } | Self::LoginAddress)
    }

    /// Whether this bucket is counted separately per client address.
    ///
    /// `LoginAccount` is not, and that is the whole of its value: the bucket
    /// already names the account, and an attacker who can vary their source
    /// address cannot vary the account they are trying to get into. Counting
    /// it per address would hand them the full allowance again from every
    /// address they can reach the instance from, which for anything behind a
    /// residential connection or a botnet is unbounded (PRD §21.4.3).
    ///
    /// `Api` is not either, for the mirror reason: it already names the
    /// credential, and adding the address would split one operator's budget
    /// across their devices while merging every caller behind an untrusted
    /// proxy into one.
    #[must_use]
    pub const fn counts_per_address(&self) -> bool {
        !matches!(self, Self::LoginAccount { .. } | Self::Api { .. })
    }
}

/// A limit: how many, over how long, and what happens after.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// How many are permitted inside the window.
    pub allowance: u32,
    /// The window, in milliseconds.
    pub window_millis: i64,
    /// The escalating lockout applied once the allowance is spent.
    pub lockout: Option<Lockout>,
}

/// An exponential lockout, bounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lockout {
    /// The first lockout's length.
    pub initial_millis: i64,
    /// The longest a lockout ever gets, however many times it doubles.
    pub ceiling_millis: i64,
}

impl Lockout {
    /// The length of the `n`-th consecutive lockout, one-based.
    ///
    /// Doubling, capped. The cap is the point: an unbounded doubling locks a
    /// household out of its own instance for a year over a fat-fingered
    /// password, which is a denial of service the attacker did not have to
    /// build.
    #[must_use]
    pub fn duration_millis(&self, consecutive: u32) -> i64 {
        let doublings = if consecutive == 0 { 0 } else { consecutive - 1 };
        let Some(shift) = self.initial_millis.checked_shl(doublings.min(63)) else {
            return self.ceiling_millis;
        };
        if shift <= 0 || shift > self.ceiling_millis {
            self.ceiling_millis
        } else {
            shift
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The name a login bucket ended up keyed on.
    fn keyed_name(bucket: &Bucket) -> String {
        match bucket {
            Bucket::LoginAccount { username } => username.clone(),
            _ => panic!("the constructor must build its own variant"),
        }
    }

    #[test]
    fn a_name_within_the_bound_is_kept_whole() {
        assert_eq!(keyed_name(&Bucket::login_account("operator")), "operator");
    }

    #[test]
    fn an_oversized_name_is_bounded_rather_than_kept() {
        let keyed = keyed_name(&Bucket::login_account(&"a".repeat(100_000)));
        assert_eq!(keyed.len(), KEYED_USERNAME_BYTES);
    }

    #[test]
    fn a_bounded_name_is_cut_on_a_character_boundary() {
        // Cutting mid-codepoint would panic on the slice, which is the sign-in
        // route falling over on a name somebody can actually type.
        let keyed = keyed_name(&Bucket::login_account(&"é".repeat(1_000)));
        assert!(keyed.len() <= KEYED_USERNAME_BYTES);
        assert!(keyed.chars().all(|character| character == 'é'));
    }

    #[test]
    fn two_names_that_differ_within_the_bound_stay_separate_buckets() {
        // The bound must not merge accounts a person could really hold: two
        // failures against different names are two counts, not one.
        assert_ne!(
            Bucket::login_account("operator"),
            Bucket::login_account("operator2")
        );
    }

    #[test]
    fn the_limit_table_matches_the_one_in_the_requirements() {
        assert_eq!(
            Bucket::LoginAccount {
                username: "operator".to_owned()
            }
            .policy()
            .allowance,
            5
        );
        assert_eq!(Bucket::LoginAddress.policy().allowance, 20);
        assert_eq!(Bucket::SetupAttempt.policy().allowance, 5);
        assert_eq!(Bucket::api("session").policy().allowance, 600);
        assert_eq!(Bucket::api("session").policy().window_millis, 60 * 1000);
        assert_eq!(Bucket::Anonymous.policy().allowance, 300);
        assert_eq!(Bucket::Provider.policy().allowance, 60);
    }

    #[test]
    fn two_credentials_are_two_api_budgets() {
        // One operator's browser and one integration's key are two callers, and
        // neither may exhaust the other's allowance.
        assert_ne!(Bucket::api("a-session-digest"), Bucket::api("a-key-id"));
    }

    #[test]
    fn anonymous_traffic_never_lands_in_an_api_budget() {
        // The lockout this closes is the whole interface: an unauthenticated
        // flood that spent `Api` would answer 429 to the operator's own
        // dashboard for the rest of the window.
        assert_ne!(Bucket::Anonymous, Bucket::api(""));
    }

    #[test]
    fn only_the_login_buckets_count_failures_only() {
        assert!(
            Bucket::LoginAccount {
                username: "operator".to_owned()
            }
            .counts_failures_only()
        );
        assert!(Bucket::LoginAddress.counts_failures_only());
        assert!(!Bucket::api("session").counts_failures_only());
        assert!(!Bucket::Anonymous.counts_failures_only());
        assert!(!Bucket::Provider.counts_failures_only());
        assert!(!Bucket::SetupAttempt.counts_failures_only());
    }

    #[test]
    fn only_the_buckets_that_name_their_caller_are_counted_without_an_address() {
        // Each already names who is being counted. Adding the address to the
        // key would give a guesser five attempts per address instead of five in
        // total, and would split one operator's API budget across their
        // devices while merging every caller behind an untrusted proxy.
        assert!(
            !Bucket::LoginAccount {
                username: "operator".to_owned()
            }
            .counts_per_address()
        );
        assert!(!Bucket::api("a-session-digest").counts_per_address());
        assert!(Bucket::LoginAddress.counts_per_address());
        assert!(Bucket::SetupAttempt.counts_per_address());
        assert!(Bucket::Anonymous.counts_per_address());
        assert!(Bucket::Provider.counts_per_address());
    }

    #[test]
    fn the_lockout_doubles_from_fifteen_minutes() {
        let lockout = Lockout {
            initial_millis: 15 * 60 * 1000,
            ceiling_millis: 24 * 60 * 60 * 1000,
        };
        assert_eq!(lockout.duration_millis(1), 15 * 60 * 1000);
        assert_eq!(lockout.duration_millis(2), 30 * 60 * 1000);
        assert_eq!(lockout.duration_millis(3), 60 * 60 * 1000);
    }

    #[test]
    fn the_lockout_stops_doubling_at_twenty_four_hours() {
        let lockout = Lockout {
            initial_millis: 15 * 60 * 1000,
            ceiling_millis: 24 * 60 * 60 * 1000,
        };
        for consecutive in 8..64 {
            assert_eq!(
                lockout.duration_millis(consecutive),
                24 * 60 * 60 * 1000,
                "lockout {consecutive} exceeded the ceiling"
            );
        }
    }

    #[test]
    fn a_very_large_lockout_count_does_not_overflow_into_a_short_lockout() {
        let lockout = Lockout {
            initial_millis: 15 * 60 * 1000,
            ceiling_millis: 24 * 60 * 60 * 1000,
        };
        assert_eq!(lockout.duration_millis(u32::MAX), 24 * 60 * 60 * 1000);
    }
}
