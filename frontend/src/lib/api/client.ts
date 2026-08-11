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
	onError: () =>
		new Response(
			JSON.stringify({
				code: 'upstream',
				message: t('api.unreachable'),
			} satisfies Problem),
			{
				status: UNREACHABLE_STATUS,
				headers: { 'content-type': 'application/json' },
			},
		),
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
