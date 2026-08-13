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

const { api, asProblem, CSRF_HEADER, csrfHeaders, readCookie } = await import(
	'./client'
);

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

describe('reading a refusal that is not the documented shape', () => {
	/** What a proxy answers with when the instance behind it is down. */
	const proxyPage = () =>
		Promise.resolve(
			new Response('<html><body><h1>502 Bad Gateway</h1></body></html>', {
				status: 502,
				headers: { 'content-type': 'text/html' },
			}),
		);

	/** What axum answers with for a method a route does not serve. */
	const bodiless = () => Promise.resolve(new Response(null, { status: 405 }));

	test("a proxy's own HTML error page is read as a refusal with a sentence", async () => {
		// The failure this closes. `openapi-fetch` reads the body as text and
		// parses it as JSON only if it can, so this arrives as a bare string.
		// Asserted to be a `Problem`, its `message` is `undefined`, and the
		// operator is shown an alert box with a heading and nothing in it.
		transport = proxyPage;

		const { error } = await api.GET('/api/auth/session');
		const problem = asProblem(error);

		expect(typeof error).toBe('string');
		expect(problem.code).toBe('upstream');
		expect(problem.message.length).toBeGreaterThan(0);
	});

	test('a refusal carrying no body at all is read the same way', async () => {
		// axum's own answer for a method a route does not serve. `openapi-fetch`
		// hands back whatever reading the body produced — an empty string here,
		// `undefined` where the runtime gives it nothing to read — and the
		// unguarded `problem.code` reads behind this are what left the shell on
		// a skeleton that never resolved.
		transport = bodiless;

		const { error } = await api.POST('/api/auth/logout');
		const problem = asProblem(error);

		expect(typeof error).not.toBe('object');
		expect(problem.code).toBe('upstream');
		expect(problem.message.length).toBeGreaterThan(0);
	});

	test('a 2xx that is not JSON is a refusal rather than an exception', async () => {
		// The deployment this closes: `/api/*` is not routed to the backend — an
		// nginx `location /` SPA fallback, a captive portal, a stale service
		// worker — so a read answers `200 text/html` with the shell.
		// `openapi-fetch` runs `JSON.parse` on a 2xx body unguarded, and the
		// `SyntaxError` escapes past every wrapper's promise of a value: the
		// session stays `unknown` and the shell waits on a skeleton for ever.
		transport = () =>
			Promise.resolve(
				new Response('<!doctype html><html><body>app</body></html>', {
					status: 200,
					headers: { 'content-type': 'text/html' },
				}),
			);

		const { data, error } = await api.GET('/api/auth/session');

		expect(data).toBeUndefined();
		expect(asProblem(error).code).toBe('upstream');
		expect(asProblem(error).message.length).toBeGreaterThan(0);
	});

	test('a bodiless success is still a success', async () => {
		// 204 is what sign-out and revocation answer with, and it carries no
		// content type to judge. Reading it as unreadable would report every
		// successful sign-out as a failure.
		transport = () => Promise.resolve(new Response(null, { status: 204 }));

		const { error } = await api.POST('/api/auth/logout');

		expect(error).toBeUndefined();
	});

	test('a refusal the API really sent is passed through unchanged', () => {
		const refusal = {
			code: 'unauthenticated' as const,
			message: 'That username and password were not accepted.',
		};

		expect(asProblem(refusal)).toBe(refusal);
	});

	test('a JSON body that is not a problem is not mistaken for one', () => {
		// Anything can answer on this origin. A shape with no `code` and no
		// `message` narrows to nothing, and every caller narrows on `code`.
		for (const value of [null, 42, [], {}, { code: 7 }, { message: 'x' }]) {
			expect(asProblem(value).code).toBe('upstream');
		}
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
