// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { beforeEach, describe, expect, mock, test } from 'bun:test';
import type { AuthResult, SignedIn } from './auth-client';

/**
 * What `/api/auth/session` will answer.
 *
 * The module is replaced rather than the network, because the real one reaches
 * `$lib/api/client` — an alias `svelte-kit sync` generates, which this lane
 * runs outside of (see the note in destinations.test.ts).
 */
let answer: AuthResult<SignedIn> = {
	outcome: 'refused',
	problem: { code: 'unauthenticated', message: 'Sign in to continue.' },
} as AuthResult<SignedIn>;

mock.module('./auth-client', () => ({
	readSession: () => Promise.resolve(answer),
}));

const { createSession } = await import('./session.svelte');

const OPERATOR: SignedIn = {
	userId: 'U',
	username: 'operator',
	isAdmin: true,
};

describe('what the interface knows about who is signed in', () => {
	beforeEach(() => {
		answer = { outcome: 'ok', value: OPERATOR };
	});

	test('nothing has been asked before the first refresh', () => {
		// `unknown` is a state and not a synonym for signed out. The shell
		// renders neither itself nor the sign-in page while it holds, because
		// rendering either would flash the wrong one at an operator who is
		// already signed in (P1 — absence of evidence is not evidence of
		// absence).
		expect(createSession().state.kind).toBe('unknown');
	});

	test('an accepted credential is recorded with the account it belongs to', async () => {
		const session = createSession();
		await session.refresh();

		expect(session.state).toEqual({ kind: 'signedIn', account: OPERATOR });
	});

	test('a refusal is signed out and not unknown', async () => {
		// This is what the shell guard turns into a redirect. Leaving it at
		// `unknown` would hold an expired session on a loading state forever.
		answer = {
			outcome: 'refused',
			problem: { code: 'unauthenticated', message: 'Sign in to continue.' },
		} as AuthResult<SignedIn>;

		const session = createSession();
		await session.refresh();

		expect(session.state.kind).toBe('signedOut');
	});

	test('a sign-in that just happened is adopted without a second round trip', () => {
		const session = createSession();
		session.adopt(OPERATOR);

		expect(session.state).toEqual({ kind: 'signedIn', account: OPERATOR });
	});

	test('the privilege the account carries is the one that is kept', () => {
		// A linked Plex account that administers nothing must not be recorded
		// as an administrator: the shell routes on this, and admin-only pages
		// answer 403 to it.
		const viewer: SignedIn = {
			userId: 'V',
			username: 'viewer',
			isAdmin: false,
		};
		const session = createSession();
		session.adopt(viewer);

		expect(
			session.state.kind === 'signedIn' && session.state.account.isAdmin,
		).toBe(false);
	});

	test('signing out is remembered without asking again', () => {
		const session = createSession();
		session.adopt(OPERATOR);
		session.forget();

		expect(session.state.kind).toBe('signedOut');
	});
});
