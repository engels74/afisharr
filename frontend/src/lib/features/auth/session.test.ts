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

/** What one call to `readSession` does, so a test can hold one open. */
let respond = (): Promise<AuthResult<SignedIn>> => Promise.resolve(answer);

mock.module('./auth-client', () => ({
	readSession: () => respond(),
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
		respond = () => Promise.resolve(answer);
	});

	test('an answer in flight is not an answer', async () => {
		// What the shell guard reads before it acts on `signedOut`: the refusal
		// recorded on the route before this one is still the state during the
		// request that will replace it, and redirecting on it sends the
		// operator who has just signed in back to the sign-in page (P1).
		let land: (result: AuthResult<SignedIn>) => void = () => {};
		respond = () =>
			new Promise<AuthResult<SignedIn>>((resolve) => {
				land = resolve;
			});

		const session = createSession();
		expect(session.refreshing).toBe(false);

		const asked = session.refresh();
		expect(session.refreshing).toBe(true);

		land({ outcome: 'ok', value: OPERATOR });
		await asked;
		expect(session.refreshing).toBe(false);
		expect(session.state.kind).toBe('signedIn');
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

	test('a fault at the instance is not a sign-out', async () => {
		// The failure this closes: `/api/auth/session` answers 500 — a bad
		// deploy, a database that will not open, a gateway answering for the
		// instance — and the shell reads "not signed in", redirects, and puts
		// an operator who is still signed in on the sign-in page mid-task. The
		// cookie is not what failed, and nothing here has learned otherwise
		// (P1 — absence of evidence is not evidence of absence).
		const session = createSession();
		await session.refresh();
		expect(session.state.kind).toBe('signedIn');

		answer = {
			outcome: 'refused',
			problem: { code: 'internal', message: 'Afisharr could not do that.' },
		} as AuthResult<SignedIn>;
		await session.refresh();

		expect(session.state).toEqual({ kind: 'signedIn', account: OPERATOR });
	});

	test('a transport failure on the first ask leaves the state unknown', async () => {
		// Nothing has been asked and answered, so nothing is known. Recording
		// `signedOut` here would send every visitor to the sign-in page for as
		// long as the instance was restarting, including the ones holding a
		// perfectly good cookie.
		answer = {
			outcome: 'refused',
			problem: { code: 'upstream', message: 'Afisharr did not answer.' },
		} as AuthResult<SignedIn>;

		const session = createSession();
		await session.refresh();

		expect(session.state.kind).toBe('unknown');
	});

	test('a refusal after a fault still signs the session out', async () => {
		// The other half: keeping state through a fault must not make an
		// expired cookie unrecognisable, or the shell would sit on a session
		// the instance has already refused.
		const session = createSession();
		await session.refresh();

		answer = {
			outcome: 'refused',
			problem: { code: 'internal', message: 'Afisharr could not do that.' },
		} as AuthResult<SignedIn>;
		await session.refresh();

		answer = {
			outcome: 'refused',
			problem: { code: 'unauthenticated', message: 'Sign in to continue.' },
		} as AuthResult<SignedIn>;
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
