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
