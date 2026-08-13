// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, test } from 'bun:test';
// Relative, not `$lib`: that alias comes from `.svelte-kit/tsconfig.json`,
// which `svelte-kit sync` generates, and `bun test` runs outside the Vite
// graph. A fresh clone has no `.svelte-kit/` and would fail to resolve it.
import { en } from '../../shared/i18n/catalogue.en';
import {
	isActive,
	isBareRoute,
	landingFor,
	PRIMARY,
	SETTINGS,
	SETTINGS_SUBPAGES,
	shellFor,
} from './destinations';

describe('the navigation model', () => {
	test('there are six primary destinations plus settings', () => {
		expect(PRIMARY.length).toBe(6);
		expect(SETTINGS.href).toBe('/settings');
	});

	test('they are in the order the information architecture fixes', () => {
		expect(PRIMARY.map((entry) => entry.href)).toEqual([
			'/dashboard',
			'/collections',
			'/design',
			'/home-screen',
			'/lifecycle',
			'/doctor',
		]);
	});

	test('every label is a catalogue key that exists', () => {
		for (const entry of [...PRIMARY, SETTINGS, ...SETTINGS_SUBPAGES]) {
			expect(en[entry.label], `${entry.href} has no message`).toBeTruthy();
		}
	});

	test('every destination has a distinct href', () => {
		const hrefs = [...PRIMARY, SETTINGS, ...SETTINGS_SUBPAGES].map(
			(e) => e.href,
		);
		expect(new Set(hrefs).size).toBe(hrefs.length);
	});
});

describe('active marking', () => {
	test('an exact path is active', () => {
		expect(isActive(SETTINGS, '/settings')).toBe(true);
	});

	test('a sub-page marks its parent active', () => {
		expect(isActive(SETTINGS, '/settings/plex')).toBe(true);
	});

	test('a path that merely starts with the same characters is not active', () => {
		expect(isActive(PRIMARY[1], '/collections-archive')).toBe(false);
	});

	test('an unrelated path is not active', () => {
		expect(isActive(SETTINGS, '/doctor')).toBe(false);
	});
});

describe('where a bare visit lands', () => {
	test('an unclaimed instance boots directly to the claim page', () => {
		expect(landingFor(false, false)).toBe('/setup');
		expect(landingFor(false, true)).toBe('/setup');
	});

	test('a configured instance boots a signed-in operator to the dashboard', () => {
		expect(landingFor(true, true)).toBe('/dashboard');
	});

	test('a configured instance sends a signed-out visitor to sign in', () => {
		// `setupCompleted` is true for every visitor to a finished instance.
		// Landing on the dashboard on the strength of it puts a signed-out
		// operator inside a shell that refuses every request it makes.
		expect(landingFor(true, false)).toBe('/login');
	});

	test('the landing is one of the routes the shell actually serves', () => {
		const routed = [...PRIMARY.map((entry) => entry.href), '/setup', '/login'];
		for (const setupCompleted of [true, false]) {
			for (const signedIn of [true, false]) {
				expect(routed).toContain(landingFor(setupCompleted, signedIn));
			}
		}
	});
});

describe('which routes render without the shell', () => {
	test('the sign-in page is bare', () => {
		expect(isBareRoute('/login')).toBe(true);
	});

	test('the wizard and its steps are bare', () => {
		expect(isBareRoute('/setup')).toBe(true);
		expect(isBareRoute('/setup/admin')).toBe(true);
	});

	test('every primary destination is inside the shell', () => {
		for (const destination of PRIMARY) {
			expect(isBareRoute(destination.href)).toBe(false);
		}
		expect(isBareRoute('/settings/plex')).toBe(false);
	});

	test('a path that merely starts with the same characters is not bare', () => {
		// Otherwise a route added later at `/setup-guide` would be exempt from
		// the session guard by accident.
		expect(isBareRoute('/setup-guide')).toBe(false);
		expect(isBareRoute('/login-help')).toBe(false);
	});

	test('the landing a signed-out visitor is sent to is itself bare', () => {
		// The guard and the landing have to agree, or an operator bounces
		// between the two forever.
		expect(isBareRoute(landingFor(true, false))).toBe(true);
		expect(isBareRoute(landingFor(false, false))).toBe(true);
	});
});

describe('what a visit is allowed to see', () => {
	const administrator = {
		kind: 'signedIn',
		account: { userId: 'A', username: 'operator', isAdmin: true },
	} as const;
	const viewer = {
		kind: 'signedIn',
		account: { userId: 'V', username: 'viewer', isAdmin: false },
	} as const;

	test('an administrator gets the shell', () => {
		expect(shellFor(false, administrator)).toBe('shell');
	});

	test('an account that administers nothing does not', () => {
		// Tier 0 is admin-only (D-007). A linked Plex viewer holds a session
		// this API accepts, and routing them into the shell on the strength of
		// it gives them a navigation bar whose every page answers 403 and a
		// live stream whose handler refuses them outright.
		expect(shellFor(false, viewer)).toBe('notPermitted');
	});

	test('an account that administers nothing is not treated as signed out', () => {
		// The other half. They are signed in, so the sign-in page would take
		// them straight back here — a loop instead of a sentence.
		expect(shellFor(false, viewer)).not.toBe('waiting');
	});

	test('nothing asked yet is neither the shell nor a refusal', () => {
		expect(shellFor(false, { kind: 'unknown' })).toBe('waiting');
	});

	test('signed out waits, because the redirect is already on its way', () => {
		expect(shellFor(false, { kind: 'signedOut' })).toBe('waiting');
	});

	test('an instance that is not set up waits rather than rendering a shell', () => {
		// The dead end this closes: `/api/auth/session` answers `setupRequired`
		// on a freshly deployed container, and a state the session never
		// recorded left the shell on its loading skeleton for ever, with no
		// redirect to the wizard and no error to read (`I-UX-2`).
		expect(shellFor(false, { kind: 'setupRequired' })).toBe('waiting');
	});

	test('an instance that could not answer is an error and not a wait', () => {
		// The dead end this closes: `/api/auth/session` answers 502 during a
		// container restart, and `waiting` renders a skeleton with no
		// navigation, no sign-in link and no retry on it — so the operator sat
		// on "Still working…" until they thought to reload the page themselves.
		// A failure has to look like one (`I-UX-2`).
		expect(
			shellFor(false, {
				kind: 'unreachable',
				problem: { code: 'upstream', message: 'Afisharr did not answer.' },
			}),
		).toBe('unreachable');
	});

	test('a bare route renders itself, whoever is asking', () => {
		// The sign-in page must render for a viewer and for nobody alike, or
		// there is no way back in.
		for (const state of [
			administrator,
			viewer,
			{ kind: 'unknown' } as const,
			{ kind: 'signedOut' } as const,
			{ kind: 'setupRequired' } as const,
			{
				kind: 'unreachable',
				problem: { code: 'upstream', message: 'Afisharr did not answer.' },
			} as const,
		]) {
			expect(shellFor(true, state)).toBe('bare');
		}
	});
});
