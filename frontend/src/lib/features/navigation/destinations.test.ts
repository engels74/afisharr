// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, test } from 'bun:test';
import { en } from '$lib/shared/i18n/catalogue.en';
import {
	isActive,
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
		expect(landingFor(false)).toBe('/setup');
	});

	test('a configured instance boots to the dashboard', () => {
		expect(landingFor(true)).toBe('/dashboard');
	});

	test('the landing is one of the routes the shell actually serves', () => {
		const routed = [...PRIMARY.map((entry) => entry.href), '/setup'];
		expect(routed).toContain(landingFor(true));
		expect(routed).toContain(landingFor(false));
	});
});
