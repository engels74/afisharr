// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { afterEach, describe, expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';
import ConnectionEvidence from './connection-evidence.svelte';
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

/** The evidence block's rows, in the order they render. */
function rows(container: Element): { label: string; value: string }[] {
	const terms = [...container.querySelectorAll('dt')];
	const values = [...container.querySelectorAll('dd')];
	return terms.map((term, index) => ({
		label: term.textContent?.trim() ?? '',
		value: values[index]?.textContent?.trim() ?? '',
	}));
}

describe('the evidence a check leaves', () => {
	test('a matching identity is one row, with nothing to compare', async () => {
		// Rendering the same string twice under two labels invites the operator
		// to look for a difference that is not there.
		const screen = await render(ConnectionEvidence, {
			connection: connection({}),
		});
		expect(rows(screen.container)).toEqual([
			{ label: 'Address', value: 'http://plex.lan:32400/' },
			{ label: 'Identity', value: 'server-a' },
		]);
	});

	test('the shared prefix is dimmed and the difference is not', async () => {
		// A forty-character identifier that differs in one place is findable
		// when aligned and obvious when marked. The mark is weight, never a
		// colour: this page has no palette of its own (§24.3.5.1, D-050).
		const screen = await render(ConnectionEvidence, {
			connection: connection({
				state: 'wrongServer',
				boundMachineIdentifier: '9f2c1a77b3e84d6fa0c5e1d29b7a6f38c4e0d512',
				observedMachineIdentifier: '9f2c1a77b3e84d6fa0c5e1d29b7a6f38c4e0a512',
			}),
		});
		const marked = [
			...screen.container.querySelectorAll('dd .font-medium'),
		].map((span) => span.textContent);
		expect(marked).toEqual(['d512', 'a512']);
		const dimmed = [
			...screen.container.querySelectorAll('dd .text-muted-foreground'),
		].map((span) => span.textContent);
		expect(new Set(dimmed).size).toBe(1);
	});

	test('a matching identity marks nothing, because nothing differs', async () => {
		const screen = await render(ConnectionEvidence, {
			connection: connection({}),
		});
		expect(screen.container.querySelectorAll('dd .font-medium')).toHaveLength(
			0,
		);
	});

	test('a mismatch is two rows, adjacent and labelled by side', async () => {
		// The comparison this page exists for. Both identifiers sit in one
		// column, one directly above the other, because the operator's question
		// is character-level and cannot be answered across a paragraph.
		const screen = await render(ConnectionEvidence, {
			connection: connection({
				state: 'wrongServer',
				observedMachineIdentifier: 'server-b',
			}),
		});
		expect(rows(screen.container)).toEqual([
			{ label: 'Address', value: 'http://plex.lan:32400/' },
			{ label: 'Bound', value: 'server-a' },
			{ label: 'Answered', value: 'server-b' },
		]);
	});

	test('both identifiers share one monospace column', async () => {
		// The alignment is the design: set in different faces or different
		// columns, two opaque strings cannot be diffed by eye.
		const screen = await render(ConnectionEvidence, {
			connection: connection({
				state: 'wrongServer',
				observedMachineIdentifier: 'server-b',
			}),
		});
		const values = [...screen.container.querySelectorAll('dd')];
		const faces = values.map((value) => getComputedStyle(value).fontFamily);
		expect(new Set(faces).size).toBe(1);
		const lefts = values.map((value) => value.getBoundingClientRect().left);
		expect(new Set(lefts).size).toBe(1);
	});

	test('a fact the server did not report is absent, not blank', async () => {
		// A row reading "Version:" with nothing after it is a claim that the
		// server has no version, which is not what happened (P1).
		const screen = await render(ConnectionEvidence, {
			connection: connection({
				version: null,
				friendlyName: null,
				observedMachineIdentifier: null,
			}),
		});
		const text = screen.container.textContent ?? '';
		expect(text).not.toContain('1.41.0');
		expect(text).not.toContain('Living Room');
		expect(rows(screen.container)).toHaveLength(2);
	});
});

describe('the choice a different server forces', () => {
	test('both ways out are named, heaviest first', async () => {
		// The order is the only thing here that says they are not equivalent.
		const screen = await render(WrongServerChoices, {});
		const ways = [...screen.container.querySelectorAll('[data-way]')].map(
			(way) => way.getAttribute('data-way'),
		);
		expect(ways).toEqual(['rebind', 'restore']);
	});

	test('each way states what it costs', async () => {
		const screen = await render(WrongServerChoices, {});
		const text = screen.container.textContent ?? '';
		expect(text).toContain('Cannot be undone');
		expect(text).toContain('Keeps your work');
	});

	test('neither choice is a control this build can act on', async () => {
		// `I-ID-5` is the operator's decision. Nothing here resolves it, and a
		// button that did nothing would be the interface lying about what it
		// can do (PRD §8.6).
		const screen = await render(WrongServerChoices, {});
		expect(screen.container.querySelectorAll('button')).toHaveLength(0);
		expect(screen.container.querySelectorAll('a')).toHaveLength(0);
	});
});

describe('both modes', () => {
	afterEach(() => {
		document.documentElement.classList.remove('dark');
	});

	/** Every class the evidence block emits, under `mode`. */
	async function classesUnder(mode: 'light' | 'dark'): Promise<string[]> {
		document.documentElement.classList.toggle('dark', mode === 'dark');
		const screen = await render(ConnectionEvidence, {
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
