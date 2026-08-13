// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import createClient from 'openapi-fetch';
// Relative, not `$lib`: this module is reachable from `bun test`, which
// resolves outside the Vite graph (see the note in destinations.test.ts).
import { t } from '../shared/i18n';
import type { components, paths } from './generated/schema';

/**
 * The one way the interface reaches the API.
 *
 * Typed entirely from the generated schema: a path this instance does not serve
 * is a type error, and so is a body of the wrong shape. There is no
 * hand-written `fetch` anywhere in `src/`, because the annotations on the Rust
 * handlers are the contract and a second description of the surface is a second
 * thing to keep in step (PRD §24.5).
 *
 * `credentials: 'same-origin'` because the session is a cookie: the SPA is
 * served by the same binary that serves the API, so there is no cross-origin
 * case to configure for.
 */
export const api = createClient<paths>({
	baseUrl: '',
	credentials: 'same-origin',
});

/** The failure shape every route answers with. */
export type Problem = components['schemas']['Problem'];

/** What kind of failure a {@link Problem} is. */
export type ErrorCode = components['schemas']['ErrorCode'];

/**
 * The status a failure this client synthesised is reported under.
 *
 * 503, because that is what it describes: the instance did not answer. Nothing
 * reads it — every caller narrows on {@link Problem.code} — but a response has
 * to carry one, and one that lied about having succeeded would be worse than
 * one nobody looks at.
 */
const UNREACHABLE_STATUS = 503;

/**
 * The status a 2xx this client could not read is reported under.
 *
 * 502, matching the backend's own `upstream` code: something answered on this
 * origin, and it was not this API.
 */
const UNREADABLE_STATUS = 502;

/** A refusal this client synthesised, in the shape every route answers with. */
function problemResponse(message: string, status: number): Response {
	return new Response(
		JSON.stringify({ code: 'upstream', message } satisfies Problem),
		{ status, headers: { 'content-type': 'application/json' } },
	);
}

/**
 * Turns a 2xx that is not JSON into a refusal, before anything tries to parse it.
 *
 * The `onError` middleware below covers one failure only, and it is narrower
 * than it looks: `openapi-fetch` runs the error chain inside the `try` around
 * `fetch` itself, so it catches a request the browser could not make and
 * nothing else. Body parsing happens afterwards and unguarded — for a 2xx it
 * runs `JSON.parse` on the text — so a success status carrying something that
 * is not JSON throws a `SyntaxError` straight out of every wrapper in
 * `$lib/features`, past the promise-of-a-value contract they all document.
 *
 * The deployment that produces it is ordinary: `/api/*` is not routed to the
 * backend — an nginx `location /` SPA fallback, a captive portal, a stale
 * service worker — so `GET /api/auth/session` answers `200 text/html` with the
 * shell. `session.refresh()` has a `finally` and no `catch`, so the rejection
 * escapes as an unhandled rejection, the session stays `unknown`, and the shell
 * renders its skeleton for ever with no navigation and no retry — the `I-UX-2`
 * dead end the session module says it exists to prevent. Out of
 * `startPlexPin()` the same throw skips the line that clears `starting`, and
 * both start buttons stay disabled for the life of the page.
 *
 * A refusal is returned rather than a repaired body, because that is what
 * happened: the call was made, an answer came back, and nothing in it was this
 * API. Bodiless successes are left alone — 204 is what sign-out and revocation
 * answer with, and it is not an answer this has anything to say about.
 */
api.use({
	onResponse: ({ response }) => {
		const bodiless =
			response.status === 204 || response.headers.get('content-length') === '0';
		const json = (response.headers.get('content-type') ?? '')
			.toLowerCase()
			.includes('json');
		if (!response.ok || bodiless || json) {
			return undefined;
		}
		return problemResponse(t('api.unreadable'), UNREADABLE_STATUS);
	},
});

/**
 * Turns a transport failure into the failure shape every route answers with.
 *
 * The browser rejects `fetch` when it cannot reach the instance at all — a
 * restart, a dropped network, a proxy in between — and that rejection
 * propagates out of `openapi-fetch` as an exception. Every wrapper in
 * `$lib/features` promises a refusal *value* instead, and their callers await
 * them without a `try`, so the exception escapes past the code that would have
 * cleared the pending state: the form stays disabled, the spinner stays up, and
 * the interface waits for something that already failed.
 *
 * Normalised here rather than in each wrapper, because "the API answered
 * something" and "the API could not be reached" are both answers to the same
 * question, and a wrapper that had to remember to catch is a wrapper somebody
 * will write without catching.
 */
api.use({
	onError: () => problemResponse(t('api.unreachable'), UNREACHABLE_STATUS),
});

/**
 * Reads a refusal as the shape every route documents, whatever arrived.
 *
 * The middleware above covers one failure only: a `fetch` the browser rejected.
 * A response that arrived and was refused takes a different path, and
 * `openapi-fetch` hands its body back unnarrowed — it reads the body as text
 * and parses it as JSON *if it can*, so a non-JSON refusal stays a bare string,
 * and a refusal carrying no body at all — axum's own 405 for a method the route
 * does not serve, and every 204 — arrives as `undefined`. Neither is a
 * {@link Problem}, and every wrapper used to assert that it was.
 *
 * What the assertion cost is not a type-level complaint. Put the instance
 * behind nginx or Cloudflare and restart the container mid-request: the proxy
 * answers its own HTML 502 page, `problem.message` is `undefined`, and the
 * operator is shown an alert box with a heading and nothing in it. The
 * unguarded `problem.code` reads on the same values are worse — the session
 * refresh and the setup form both dereference it before anything has narrowed
 * it, so the shell waits on a skeleton that never resolves and the form clears
 * `submitting` and then throws past the code that would have shown why.
 *
 * The synthesised refusal says the instance answered with something this
 * interface could not read, which is the true account: the call was made, an
 * answer came back, and nothing in it named a failure.
 */
export function asProblem(refusal: unknown): Problem {
	return isProblem(refusal)
		? refusal
		: { code: 'upstream', message: t('api.unreadable') };
}

/** Whether a value carries the two fields every {@link Problem} declares. */
function isProblem(value: unknown): value is Problem {
	if (typeof value !== 'object' || value === null) {
		return false;
	}
	const candidate = value as Partial<Problem>;
	return (
		typeof candidate.code === 'string' && typeof candidate.message === 'string'
	);
}

/**
 * The cookie the API sets for the double-submit CSRF check.
 *
 * Readable by script on purpose: the check needs the page to echo it, and a
 * value the page cannot read is a value it cannot echo.
 */
const CSRF_COOKIE = 'afisharr_csrf';

/** The header the value is echoed in. */
export const CSRF_HEADER = 'x-afisharr-csrf';

/**
 * The headers a state-changing call must carry.
 *
 * Called at the point of use rather than installed as a global middleware:
 * the cookie is set by signing in and cleared by signing out, so a value read
 * once at module load would be the value from before the last sign-in.
 */
export function csrfHeaders(): Record<string, string> {
	const token = readCookie(CSRF_COOKIE);
	return token ? { [CSRF_HEADER]: token } : {};
}

/** The value of one cookie, or `undefined` when it is not set. */
export function readCookie(name: string): string | undefined {
	if (typeof document === 'undefined') {
		return undefined;
	}
	for (const entry of document.cookie.split(';')) {
		const [key, ...rest] = entry.trim().split('=');
		if (key === name) {
			return decodeURIComponent(rest.join('='));
		}
	}
	return undefined;
}
