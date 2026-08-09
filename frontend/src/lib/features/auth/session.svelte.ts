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
	/** Asks the API who is signed in. */
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
	let inFlight = $state(0);

	return {
		get state() {
			return state;
		},

		get refreshing() {
			return inFlight > 0;
		},

		async refresh(): Promise<void> {
			inFlight += 1;
			try {
				const result = await readSession();
				state =
					result.outcome === 'ok'
						? { kind: 'signedIn', account: result.value }
						: { kind: 'signedOut' };
			} finally {
				inFlight -= 1;
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
