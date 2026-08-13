// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * What the interface does about light and dark.
 *
 * The browser lane, and it has to be: the mode is a class on the document
 * element written by an effect, a value in local storage, and a media query
 * that may not exist. None of the three is observable from a unit test.
 */

import { afterEach, describe, expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';
import ModePreference from './mode-preference.svelte';
import ModeToggle from './mode-toggle.svelte';

/** Where `mode-watcher` persists the operator's choice. */
const STORAGE_KEY = 'mode-watcher-mode';

/** The document, as the next test needs to find it. */
function reset(): void {
	localStorage.removeItem(STORAGE_KEY);
	document.documentElement.classList.remove('dark');
}

/** Lets the mode effects flush and the class land. */
async function settle(): Promise<void> {
	for (let round = 0; round < 5; round += 1) {
		await new Promise((resolve) => requestAnimationFrame(resolve));
	}
}

afterEach(reset);

describe('the mode control', () => {
	test('an explicit choice is persisted, not held in memory', async () => {
		// Local storage rather than a variable, because the operator's choice has
		// to survive the next visit and local storage is what the pre-paint
		// script reads before anything renders.
		const screen = await render(ModeToggle);

		await screen.getByRole('radio', { name: 'Dark' }).click();
		await settle();

		expect(localStorage.getItem(STORAGE_KEY)).toBe('dark');
	});

	test('the way back to the system preference is offered', async () => {
		// The reason this is a radiogroup and not a toggle: a two-state control
		// spends the "follow the system" default on its first press and gives
		// the operator no way to restore it (P2).
		const screen = await render(ModeToggle);

		await screen.getByRole('radio', { name: 'Dark' }).click();
		await screen.getByRole('radio', { name: 'Follow the system' }).click();
		await settle();

		expect(localStorage.getItem(STORAGE_KEY)).toBe('system');
	});

	test('the chosen segment is the one the group reports as checked', async () => {
		const screen = await render(ModeToggle);

		await screen.getByRole('radio', { name: 'Light' }).click();
		await settle();

		const light = screen.container.querySelector('[aria-label="Light"]');
		expect(light?.getAttribute('data-state')).toBe('checked');
	});
});

describe('the mode default', () => {
	test('a stored choice from a previous visit is what renders', async () => {
		// The reload half of the requirement: nothing on this visit chose dark,
		// and the interface opens dark because the last visit did (PRD §10.4).
		localStorage.setItem(STORAGE_KEY, 'dark');

		await render(ModePreference);
		await settle();

		expect(document.documentElement.classList.contains('dark')).toBe(true);
	});

	test('a browser that cannot be asked renders light, not dark', async () => {
		// `mode-watcher` tests `(prefers-color-scheme: light)` and maps every
		// non-match to dark — including this one, where nothing was tested and
		// nothing was learned (P1). PRD §10.4 fixes the answer as light.
		const real = window.matchMedia;
		Object.defineProperty(window, 'matchMedia', {
			configurable: true,
			value: undefined,
		});
		try {
			await render(ModePreference);
			await settle();

			expect(localStorage.getItem(STORAGE_KEY)).toBe('light');
			expect(document.documentElement.classList.contains('dark')).toBe(false);
		} finally {
			Object.defineProperty(window, 'matchMedia', {
				configurable: true,
				value: real,
			});
		}
	});

	test('a browser that can be asked keeps following the system', async () => {
		await render(ModePreference);
		await settle();

		// Not `light`: setting a mode here would turn "follow the system" into
		// a stored preference and a genuine system-dark preference would stop
		// being honoured.
		expect(localStorage.getItem(STORAGE_KEY)).toBe('system');
	});
});
