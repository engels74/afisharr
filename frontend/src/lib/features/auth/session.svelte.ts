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

	return {
		get state() {
			return state;
		},

		async refresh(): Promise<void> {
			const result = await readSession();
			state =
				result.outcome === 'ok'
					? { kind: 'signedIn', account: result.value }
					: { kind: 'signedOut' };
		},

		adopt(account: SignedIn): void {
			state = { kind: 'signedIn', account };
		},

		forget(): void {
			state = { kind: 'signedOut' };
		},
	};
}
