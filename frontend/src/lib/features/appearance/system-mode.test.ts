// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, test } from 'bun:test';
import { fallbackMode, isModeChoice, MODE_CHOICES } from './system-mode';

describe('the light fallback', () => {
	test('asks the system where the query can be run', () => {
		expect(fallbackMode({ matchMedia: () => ({ matches: true }) })).toBe(
			undefined,
		);
	});

	test('renders light where the browser has no matchMedia', () => {
		// The case the library gets backwards: it tests
		// `(prefers-color-scheme: light)` and treats every non-match as dark, so
		// a browser that answered nothing at all lands in dark by accident
		// (PRD §10.4, D-050).
		expect(fallbackMode({})).toBe('light');
	});

	test('renders light where matchMedia is present but not callable', () => {
		expect(fallbackMode({ matchMedia: null })).toBe('light');
		expect(fallbackMode({ matchMedia: 'yes' })).toBe('light');
	});

	test('renders light where there is no window at all', () => {
		expect(fallbackMode(undefined)).toBe('light');
	});
});

describe('the choices', () => {
	test('are the three mode-watcher persists', () => {
		expect([...MODE_CHOICES]).toEqual(['system', 'light', 'dark']);
	});

	test('narrow a string that is one of them', () => {
		for (const choice of MODE_CHOICES) {
			expect(isModeChoice(choice)).toBe(true);
		}
	});

	test('reject a string that is not', () => {
		// The radio group hands back a string, and an empty one is what a
		// deselect looks like. Setting the mode from it would clear a choice
		// the operator never cleared.
		for (const value of ['', 'System', 'sepia']) {
			expect(isModeChoice(value), value).toBe(false);
		}
	});
});
