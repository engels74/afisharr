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
export class Session {
	state = $state<SessionState>({ kind: 'unknown' });

	/** Asks the API who is signed in. */
	async refresh(): Promise<void> {
		const result = await readSession();
		this.state =
			result.outcome === 'ok'
				? { kind: 'signedIn', account: result.value }
				: { kind: 'signedOut' };
	}

	/** Records a sign-in that just happened, without a second round trip. */
	adopt(account: SignedIn): void {
		this.state = { kind: 'signedIn', account };
	}

	/** Records a sign-out. */
	forget(): void {
		this.state = { kind: 'signedOut' };
	}
}
