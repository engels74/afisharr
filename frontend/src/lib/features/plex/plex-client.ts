// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import {
	type ApiSchemas,
	api,
	asProblem,
	csrfHeaders,
	type Problem,
} from '$lib/api/client';

/** What the check saw, exactly as the API declares it. */
export type PlexConnection = ApiSchemas['PlexConnection'];

/** Where the connection stands. */
export type PlexConnectionState = ApiSchemas['PlexConnectionState'];

/** What the check produced. Never an exception: a refusal is a value. */
export type ConnectionResult =
	| { readonly outcome: 'ok'; readonly value: PlexConnection }
	| { readonly outcome: 'refused'; readonly problem: Problem };

/**
 * Asks the instance to check its Plex connection.
 *
 * A `POST` because it is an act: it makes a request to the operator's server
 * and records what it saw. There is deliberately no cached-state read beside
 * it — a page that showed a stored verdict without saying when it was taken
 * would report a server as reachable long after it stopped answering.
 */
export async function checkConnection(): Promise<ConnectionResult> {
	const { data, error } = await api.POST(
		'/api/settings/plex/connection/check',
		{ headers: csrfHeaders() },
	);
	return data
		? { outcome: 'ok', value: data }
		: { outcome: 'refused', problem: asProblem(error) };
}

/**
 * Whether this state stops everything until an operator decides.
 *
 * Read from the state the API returned, never from the status code the call
 * arrived under: the check succeeds — it did check — and what it found is in
 * the body (`I-UX-2`).
 */
export function blocks(state: PlexConnectionState): boolean {
	return state === 'wrongServer';
}
