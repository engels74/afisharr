// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { readSession, type SignedIn } from './auth-client';

/** What the interface knows about who is signed in. */
export type SessionState =
	| { readonly kind: 'unknown' }
	| { readonly kind: 'signedIn'; readonly account: SignedIn }
	| { readonly kind: 'signedOut' };

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
	 * Records an answer, and only an answer: a refused credential is a
	 * sign-out, and every other failure leaves what is known where it was.
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

	return {
		get state() {
			return state;
		},

		get refreshing() {
			return refreshing;
		},

		async refresh(): Promise<void> {
			inFlight += 1;
			refreshing = true;
			try {
				const result = await readSession();
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
				if (result.problem.code === 'unauthenticated') {
					state = { kind: 'signedOut' };
				}
			} finally {
				inFlight -= 1;
				refreshing = inFlight > 0;
			}
		},

		adopt(account: SignedIn): void {
			state = { kind: 'signedIn', account };
		},

		forget(): void {
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
