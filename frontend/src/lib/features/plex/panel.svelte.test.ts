// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { ConnectionResult, PlexConnection } from './plex-client';

/**
 * The panel mounted, with the one call it makes answered.
 *
 * This suite exists because of a bug nothing else caught. The load effect
 * called a function that reads `checking` for its own re-entry guard, and a
 * read inside an effect is a subscription — so the effect re-ran the moment the
 * guard flipped and the panel rendered nothing at all: no state, no error, no
 * empty treatment. Every other test in this feature passed, because none of
 * them mounted the panel. A screenshot of the running page is what found it.
 *
 * The client module is the seam. `globalThis.fetch` is not: `openapi-fetch`
 * captures it when the client is constructed, which happens on import, so a
 * spy installed afterwards is a spy the client never sees. What broke here was
 * the wiring between the panel and its effect, and this exercises all of it.
 */

let answer: PlexConnection;
const checked = vi.fn<() => Promise<ConnectionResult>>();

vi.mock('./plex-client', async (original) => ({
	...(await original<typeof import('./plex-client')>()),
	checkConnection: () => checked(),
}));

const { default: ConnectionPanel } = await import('./connection-panel.svelte');

function answering(body: Partial<PlexConnection>) {
	answer = {
		state: 'reachable',
		baseUrl: 'http://plex.lan:32400/',
		boundMachineIdentifier: 'server-a',
		observedMachineIdentifier: 'server-a',
		friendlyName: 'Living Room',
		version: '1.41.0',
		detail: null,
		checkedAt: 1_700_000_000_000,
		...body,
	};
}

/** Waits for the panel's one call to settle and the DOM to catch up. */
async function settle() {
	await new Promise((resolve) => setTimeout(resolve, 50));
}

describe('the connection panel', () => {
	beforeEach(() => {
		checked.mockReset();
		checked.mockImplementation(() =>
			Promise.resolve({ outcome: 'ok', value: answer }),
		);
		answering({});
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	test('it renders a state, and asks exactly once', async () => {
		const screen = await render(ConnectionPanel, {});
		await settle();

		// The regression: anything at all below the heading.
		expect(
			screen.container.querySelector('[data-slot="connection-evidence"]'),
		).not.toBeNull();
		expect(screen.container.textContent).toContain('server-a');

		// And once. A re-entrant effect spent the per-address provider budget
		// on every render.
		expect(checked).toHaveBeenCalledTimes(1);
	});

	test('a different server renders the blocked treatment and the choices', async () => {
		answering({ state: 'wrongServer', observedMachineIdentifier: 'server-b' });
		const screen = await render(ConnectionPanel, {});
		await settle();

		expect(
			screen.container.querySelector('[data-slot="blocked-state"]'),
		).not.toBeNull();
		expect(
			screen.container.querySelector('[data-slot="wrong-server-choices"]'),
		).not.toBeNull();
		const text = screen.container.textContent ?? '';
		expect(text).toContain('server-a');
		expect(text).toContain('server-b');
	});

	test('an installation with no server renders the empty treatment', async () => {
		answering({
			state: 'notConfigured',
			baseUrl: null,
			boundMachineIdentifier: null,
			observedMachineIdentifier: null,
			friendlyName: null,
			version: null,
		});
		const screen = await render(ConnectionPanel, {});
		await settle();

		const empty = screen.container.querySelector('[data-slot="empty-state"]');
		expect(empty).not.toBeNull();
		expect(empty?.getAttribute('data-reason')).toBe('nothingCreated');
	});

	test('a refused check renders the error treatment, not a blank panel', async () => {
		checked.mockImplementation(() =>
			Promise.resolve({
				outcome: 'refused',
				problem: { code: 'rateLimited', message: 'Too many checks.' },
			}),
		);
		const screen = await render(ConnectionPanel, {});
		await settle();

		expect(
			screen.container.querySelector('[data-slot="error-state"]'),
		).not.toBeNull();
		expect(screen.container.textContent).toContain('Too many checks.');
	});
});
