// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { beforeEach, describe, expect, test } from 'bun:test';

/** What the browser does while the instance is down. */
const unreachable = () => Promise.reject(new TypeError('Failed to fetch'));

/** What it does when the instance is up. */
const answering = () =>
	Promise.resolve(
		new Response(JSON.stringify({ version: '0.1.0', setupCompleted: true }), {
			status: 200,
			headers: { 'content-type': 'application/json' },
		}),
	);

let transport: () => Promise<Response> = answering;

// Substituted before the import, not after: `openapi-fetch` reads
// `globalThis.fetch` once, when the client is created, so a stub installed
// afterwards would never be called.
// biome-ignore lint/suspicious/noExplicitAny: substituting a browser global
(globalThis as any).fetch = () => transport();

const { api, CSRF_HEADER, csrfHeaders, readCookie } = await import('./client');

describe('reaching the instance', () => {
	test('an answer is returned as data', async () => {
		transport = answering;

		const { data, error } = await api.GET('/api/health');

		expect(error).toBeUndefined();
		expect(data?.version).toBe('0.1.0');
	});

	test('a browser that cannot reach the instance gets a refusal, not an exception', async () => {
		// The failure this closes: `fetch` rejects — a restart, a dropped
		// network, a proxy in between — and the rejection propagates out of
		// `openapi-fetch`. Every wrapper above this promises a refusal value,
		// and their callers await them without a `try`, so the exception
		// escapes past the code that clears the pending state: the form stays
		// disabled and the spinner stays up over something that already failed.
		transport = unreachable;

		const { data, error } = await api.GET('/api/auth/session');

		expect(data).toBeUndefined();
		expect(error?.code).toBe('upstream');
		expect(error?.message.length).toBeGreaterThan(0);
	});

	test('a write that cannot be delivered refuses the same way', async () => {
		// Every method goes through the one client, so a caller never has to
		// know which of them can throw.
		transport = unreachable;

		const { data, error } = await api.POST('/api/auth/logout');

		expect(data).toBeUndefined();
		expect(error?.code).toBe('upstream');
	});
});

/** Writes one cookie, which is the only thing the browser gives us. */
function setCookie(value: string): void {
	// biome-ignore lint/suspicious/noDocumentCookie: this is what the code under test reads
	document.cookie = value;
}

describe('the double-submit CSRF token', () => {
	beforeEach(() => {
		for (const entry of document.cookie.split(';')) {
			const name = entry.trim().split('=')[0];
			if (name) {
				setCookie(`${name}=; max-age=0`);
			}
		}
	});

	test('the token is read from the cookie and echoed in the header', () => {
		// Read at the point of use, never once at module load: the cookie is
		// set by signing in and cleared by signing out, so a value cached at
		// import is the value from before the last sign-in.
		setCookie('afisharr_csrf=a-token');

		expect(csrfHeaders()).toEqual({ [CSRF_HEADER]: 'a-token' });
	});

	test('no cookie means no header rather than an empty one', () => {
		// An empty header is a value the API would have to judge; no header is
		// a request that never claimed to carry one.
		expect(csrfHeaders()).toEqual({});
	});

	test('a value is decoded, and one cookie is not read as another', () => {
		setCookie('afisharr_csrf_other=wrong');
		setCookie('afisharr_csrf=a%2Ftoken');

		expect(readCookie('afisharr_csrf')).toBe('a/token');
		expect(readCookie('afisharr_nothing')).toBeUndefined();
	});
});
