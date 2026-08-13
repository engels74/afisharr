// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { afterEach, describe, expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';
import ConnectionFacts from './connection-facts.svelte';
import type { PlexConnection } from './plex-client';
import WrongServerChoices from './wrong-server-choices.svelte';

/**
 * The two pieces of the connection surface that render a verdict.
 *
 * The panel itself is not rendered here: it calls the generated client on
 * mount, and a component test that stubbed the network would be a test of the
 * stub. What the panel decides — which treatment each state gets — is covered
 * against the running instance in `backend/crates/afisharr/tests`, where the
 * answer comes from a real API and a real adversarial fake.
 */

function connection(overrides: Partial<PlexConnection>): PlexConnection {
	return {
		state: 'reachable',
		baseUrl: 'http://plex.lan:32400/',
		boundMachineIdentifier: 'server-a',
		observedMachineIdentifier: 'server-a',
		friendlyName: 'Living Room',
		version: '1.41.0',
		detail: null,
		checkedAt: 1_700_000_000_000,
		...overrides,
	};
}

describe('the facts a check observed', () => {
	test('every fact the server reported is shown', async () => {
		const screen = await render(ConnectionFacts, {
			connection: connection({}),
		});
		const text = screen.container.textContent ?? '';
		expect(text).toContain('http://plex.lan:32400/');
		expect(text).toContain('Living Room');
		expect(text).toContain('1.41.0');
		expect(text).toContain('server-a');
	});

	test('a fact the server did not report is absent, not blank', async () => {
		// A row reading "Version:" with nothing after it is a claim that the
		// server has no version, which is not what happened (P1).
		const screen = await render(ConnectionFacts, {
			connection: connection({ version: null, friendlyName: null }),
		});
		const text = screen.container.textContent ?? '';
		expect(text).not.toContain('Version');
		expect(text).not.toContain('Server');
		expect(text).toContain('http://plex.lan:32400/');
	});
});

describe('the choice a different server forces', () => {
	const blocked = connection({
		state: 'wrongServer',
		observedMachineIdentifier: 'server-b',
	});

	test('both ways out are named', async () => {
		const screen = await render(WrongServerChoices, { connection: blocked });
		const text = screen.container.textContent ?? '';
		expect(text).toContain('This is a new server — rebind');
		expect(text).toContain('Restore a backup');
	});

	test('both identifiers are named, so the decision can be made', async () => {
		// An answer naming only the stranger tells the operator nothing about
		// what they are being asked to abandon.
		const screen = await render(WrongServerChoices, { connection: blocked });
		const text = screen.container.textContent ?? '';
		expect(text).toContain('server-a');
		expect(text).toContain('server-b');
	});

	test('neither choice is a control this build can act on', async () => {
		// `I-ID-5` is the operator's decision. Nothing here resolves it, and a
		// button that did nothing would be the interface lying about what it
		// can do (PRD §8.6).
		const screen = await render(WrongServerChoices, { connection: blocked });
		expect(screen.container.querySelectorAll('button')).toHaveLength(0);
		expect(screen.container.querySelectorAll('a')).toHaveLength(0);
	});
});

describe('both modes', () => {
	afterEach(() => {
		document.documentElement.classList.remove('dark');
	});

	/** Every class the component emits, under `mode`. */
	async function classesUnder(mode: 'light' | 'dark'): Promise<string[]> {
		document.documentElement.classList.toggle('dark', mode === 'dark');
		const screen = await render(WrongServerChoices, {
			connection: connection({
				state: 'wrongServer',
				observedMachineIdentifier: 'server-b',
			}),
		});
		return [...screen.container.querySelectorAll('*')]
			.flatMap((element) => [...element.classList])
			.sort();
	}

	test('the markup does not branch on the mode', async () => {
		// A component that emitted different classes per mode is a component
		// with two appearances to keep in step, and the second one is the one
		// nobody looks at.
		expect(await classesUnder('dark')).toEqual(await classesUnder('light'));
	});

	test('every colour it names is a semantic token', async () => {
		// What makes both modes correct here: the component chooses no colour of
		// its own, so it renders whatever the mode defines. A literal — a hex, a
		// bare `oklch(...)`, a numbered palette class — is the failure that
		// reads correctly in one mode and vanishes in the other (D-050).
		const palette =
			/^(bg|text|border|fill|stroke|ring|from|via|to)-(red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose|slate|gray|grey|zinc|neutral|stone)-\d{2,3}$/;
		for (const name of await classesUnder('light')) {
			expect(name).not.toMatch(palette);
			expect(name).not.toMatch(/#[0-9a-fA-F]{3,8}|oklch\(|rgba?\(|hsla?\(/);
		}
	});
});
