// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * What the shell does on the way in.
 *
 * The browser lane, and it has to be: both failures this covers are runtime
 * behaviour of real effects. An effect that invalidates itself renders nothing
 * at all, and no amount of unit-testing the pieces it calls would show it.
 */

import { describe, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

/** Every request the layout made, in order. */
const asked: string[] = [];

/** Who `/api/auth/session` says is here. */
let account: { userId: string; username: string; isAdmin: boolean } | undefined;

// Installed before the layout is imported: `openapi-fetch` reads
// `globalThis.fetch` when the client is created, so a stub installed afterwards
// would never be called.
const realFetch = globalThis.fetch;
const stubbed = (input: RequestInfo | URL, init?: RequestInit) => {
	// `openapi-fetch` hands `fetch` a `Request`, not a string.
	const href =
		input instanceof Request
			? input.url
			: new URL(String(input), location.href).href;
	const path = new URL(href).pathname;
	asked.push(path);

	if (path === '/api/auth/session') {
		return Promise.resolve(
			account
				? json(account)
				: json({ code: 'unauthenticated', message: 'Sign in.' }, 401),
		);
	}
	if (path === '/api/health') {
		return Promise.resolve(json({ version: '0.1.0', setupCompleted: true }));
	}
	return realFetch(input, init);
};
// `Object.assign` rather than a bare function: `typeof fetch` carries
// `preconnect`, and the layout needs the one property the interface uses.
globalThis.fetch = Object.assign(stubbed, realFetch);

function json(body: unknown, status = 200): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { 'content-type': 'application/json' },
	});
}

/** Stream connections the layout opened. */
const opened: string[] = [];

class SilentEventSource {
	constructor(url: string) {
		opened.push(url);
	}
	addEventListener() {}
	close() {}
}
// biome-ignore lint/suspicious/noExplicitAny: substituting a browser global
(globalThis as any).EventSource = SilentEventSource;

vi.mock('$app/navigation', () => ({ goto: () => Promise.resolve() }));
vi.mock('$app/state', () => ({
	page: { url: new URL('http://localhost/dashboard') },
}));

const Layout = (await import('./+layout.svelte')).default;
const { session } = await import('$lib/features/auth');

/** Lets the layout's effects run and its requests land. */
async function settle(): Promise<void> {
	for (let round = 0; round < 10; round += 1) {
		await new Promise((resolve) => setTimeout(resolve, 20));
	}
}

describe('entering the shell', () => {
	test('the session is asked once, not on a loop', async () => {
		// The blocker this covers: the layout refreshed the session from an
		// `$effect`, the refresh touched the session's own in-flight counter,
		// and reading it inside the effect made it a dependency — so the write
		// re-ran the effect, which refreshed again. The shell floods
		// `/api/auth/session` and hits Svelte's `effect_update_depth_exceeded`
		// guard instead of rendering anything at all.
		asked.length = 0;
		account = { userId: 'A', username: 'operator', isAdmin: true };
		session.forget();

		const screen = await render(Layout);
		await settle();

		const sessionCalls = asked.filter(
			(path) => path === '/api/auth/session',
		).length;
		expect(sessionCalls).toBe(1);
		await expect.element(screen.getByText('Dashboard')).toBeVisible();
	});

	test('an account that administers nothing gets a sentence, not the shell', async () => {
		// A linked Plex viewer holds a session this API accepts. Routed into
		// the shell on the strength of it, they get a navigation bar whose
		// every page answers 403 and a stream whose handler requires an
		// administrator (D-007).
		asked.length = 0;
		opened.length = 0;
		account = { userId: 'V', username: 'viewer', isAdmin: false };
		session.forget();

		const screen = await render(Layout);
		await settle();

		await expect.element(screen.getByText(/administrator-only/)).toBeVisible();
		expect(
			screen.container.querySelector('nav[aria-label]'),
			'a viewer must not be given the administrator shell',
		).toBeNull();
		expect(opened, 'the stream is admin-only and must not be opened').toEqual(
			[],
		);
	});
});
