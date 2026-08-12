// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import type { Problem } from '$lib/api/client';
import { readSession, type SignedIn } from './auth-client';

/** What the interface knows about who is signed in. */
export type SessionState =
	| { readonly kind: 'unknown' }
	| { readonly kind: 'signedIn'; readonly account: SignedIn }
	| { readonly kind: 'signedOut' }
	/**
	 * The instance has not been set up, so nobody can be signed in yet.
	 *
	 * A separate state and not a synonym for signed out: the sign-in page
	 * cannot work either until setup finishes, so sending an operator there
	 * would only move the dead end. `/api/auth/session` sits behind
	 * `require_setup_completed`, which answers `setupRequired` while
	 * `instance.setup_completed_at` is `NULL` — the exact case a bookmarked
	 * `/dashboard` on a freshly deployed container lands in.
	 */
	| { readonly kind: 'setupRequired' }
	/**
	 * The instance was asked and did not answer usefully.
	 *
	 * Not `unknown`, and not `signedOut`. `unknown` means nothing has been
	 * asked yet, which the shell renders as a loading skeleton with no error
	 * and no control on it — so recording a 500, a proxy's 502, a 429, or the
	 * client's own `upstream` 503 as `unknown` left the operator on "Still
	 * working…" for ever, with no navigation rendered to leave by and nothing
	 * on screen to retry (`I-UX-2`). `signedOut` is worse: none of those says
	 * anything about the cookie, and acting on them signs out an operator
	 * mid-task over a container restart.
	 *
	 * The refusal is carried, because the sentence the instance sent is the
	 * only true account of what happened.
	 */
	| { readonly kind: 'unreachable'; readonly problem: Problem };

/**
 * Who is signed in, as one value the shell reads.
 *
 * `unknown` is a state and not a synonym for signed out: on the first load
 * nothing has been asked yet, and rendering the sign-in page during that
 * moment would flash it in front of an operator who is already signed in
 * (P1 — absence of evidence is not evidence of absence).
 */
export interface Session {
	/** What is known right now. */
	readonly state: SessionState;
	/**
	 * Whether an answer is in flight.
	 *
	 * The shell guard reads this before it acts on `signedOut`, because a
	 * refusal recorded on the previous route is still the state during the
	 * request that will replace it — and redirecting on it sends the operator
	 * who has just signed in straight back to the sign-in page.
	 */
	readonly refreshing: boolean;
	/**
	 * Asks the API who is signed in.
	 *
	 * A refused credential is a sign-out; a setup gate is a setup gate. Any
	 * other failure is recorded as `unreachable`, and only over a state that
	 * knows nothing: a session already known to be signed in survives a fault,
	 * because the cookie is not what failed, while a first ask that fails stops
	 * leaving `unknown` behind — the one state the shell can neither render nor
	 * leave.
	 */
	refresh(): Promise<void>;
	/** Records a sign-in that just happened, without a second round trip. */
	adopt(account: SignedIn): void;
	/** Records a sign-out. */
	forget(): void;
}

/**
 * A session that knows nothing yet.
 *
 * The state is closed over rather than exposed as a settable field,
 * deliberately: the three transitions are the only ways in, and each of them
 * names the evidence it acted on. A settable field would let any component
 * declare somebody signed in.
 */
export function createSession(): Session {
	let state = $state<SessionState>({ kind: 'unknown' });
	// Plain, and reactive only through the flag below. `refresh()` reads this
	// to increment it, and a rune read inside an effect is a dependency of that
	// effect — so a shell that refreshed the session from an `$effect` would
	// invalidate itself on its own write, refresh again, and flood
	// `/api/auth/session` until Svelte's update-depth guard stopped it
	// rendering entirely (P1).
	let inFlight = 0;
	let refreshing = $state(false);
	// Which ask is the current one. Two refreshes can be out at once — the
	// layout starts one on every navigation into the shell, and the
	// `unreachable` retry starts another — and nothing makes the network
	// answer them in the order they were sent. Without this, the *slower*
	// response wrote the state last: a first ask that stalls through a
	// container restart and comes back `unauthenticated` overwrote the
	// `signedIn` a later ask had already recorded, and the layout's guard sent
	// an operator who is signed in, mid-task, to the sign-in page (P1).
	let latest = 0;

	return {
		get state() {
			return state;
		},

		get refreshing() {
			return refreshing;
		},

		async refresh(): Promise<void> {
			latest += 1;
			const ask = latest;
			inFlight += 1;
			refreshing = true;
			try {
				const result = await readSession();
				// A newer ask has already answered, so this one is history and
				// writing it would undo the newer answer. The `finally` below
				// still runs, so the in-flight count stays honest.
				if (ask !== latest) {
					return;
				}
				if (result.outcome === 'ok') {
					state = { kind: 'signedIn', account: result.value };
					return;
				}
				// Only a refused credential is a signed-out one. A 500, a
				// gateway that answered for the instance, or a browser that
				// could not reach it at all say nothing about the cookie — and
				// turning any of them into `signedOut` sends an operator who is
				// still signed in to the sign-in page, mid-task, on a fault
				// that had nothing to do with them (P1).
				//
				// `setupRequired` is the third answer this route really gives,
				// and leaving it out of the record was a dead end rather than a
				// missing branch: the state stayed `unknown`, the shell rendered
				// its loading skeleton, and nothing ever replaced it. An
				// operator opening a bookmarked shell route on a container that
				// has not been set up watched it wait for ever, with no
				// redirect and no error (`I-UX-2`).
				//
				// Every remaining refusal is recorded too, but only over a state
				// that knows nothing. The two cases are different failures:
				//
				// - Already signed in, and a background refresh hits a 500.
				//   Keep it. The cookie is not what failed, and tearing the
				//   shell down over a five-second restart takes an operator off
				//   the page they were working on.
				// - Nothing known, and the first ask fails. `unknown` is the
				//   state the shell draws a skeleton for — no navigation, no
				//   sign-in link, no retry — so leaving it standing turned one
				//   502 into a wait with no end and nothing on screen to act on
				//   (`I-UX-2`). `unreachable` carries the instance's own
				//   sentence and the layout gives it a retry.
				if (result.problem.code === 'unauthenticated') {
					state = { kind: 'signedOut' };
				} else if (result.problem.code === 'setupRequired') {
					state = { kind: 'setupRequired' };
				} else if (state.kind === 'unknown' || state.kind === 'unreachable') {
					state = { kind: 'unreachable', problem: result.problem };
				}
			} finally {
				inFlight -= 1;
				refreshing = inFlight > 0;
			}
		},

		// Both retire whatever is in flight, for the same reason `refresh()`
		// checks: each of these acted on evidence newer than any answer already
		// on the wire. A sign-in adopted while the layout's ask was still out
		// was overwritten by that ask's `unauthenticated` — the cookie it was
		// sent without — and the operator was bounced straight back to the
		// sign-in page they had just completed.
		adopt(account: SignedIn): void {
			latest += 1;
			state = { kind: 'signedIn', account };
		},

		forget(): void {
			latest += 1;
			state = { kind: 'signedOut' };
		},
	};
}

/**
 * The session this document is signed in with.
 *
 * One value at module scope, because a session is a fact about the browsing
 * context rather than about a component. The two halves of a sign-in happen in
 * different places — the login route learns who the operator is, the layout
 * decides what to render — and a session held per component would leave the
 * layout redirecting on the refusal it recorded before the sign-in it never
 * heard about (P1). Same shape, and the same reason, as the provenance the
 * footer reads.
 *
 * {@link createSession} stays exported for tests, which need one that starts
 * with nothing asked.
 */
export const session = createSession();
