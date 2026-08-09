// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import createClient from 'openapi-fetch';
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
