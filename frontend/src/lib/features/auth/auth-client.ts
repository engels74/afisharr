// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { api, asProblem, csrfHeaders, type Problem } from '$lib/api/client';

/** The signed-in account. */
export interface SignedIn {
	userId: string;
	username: string;
	displayName?: string | null;
	isAdmin: boolean;
}

/** What an auth call produced. A refusal is a value, never an exception. */
export type AuthResult<T> =
	| { readonly outcome: 'ok'; readonly value: T }
	| { readonly outcome: 'refused'; readonly problem: Problem };

/** Signs in with a username and a password. */
export async function signIn(
	username: string,
	password: string,
): Promise<AuthResult<SignedIn>> {
	const { data, error } = await api.POST('/api/auth/login', {
		body: { username, password },
		headers: csrfHeaders(),
	});
	return data
		? { outcome: 'ok', value: data as SignedIn }
		: { outcome: 'refused', problem: asProblem(error) };
}

/**
 * Signs out, revoking the session that made the request.
 *
 * Returns the answer like every other call here, because this one can be
 * refused: it sits behind the setup gate, behind the anonymous rate limit, and
 * behind the cross-site check, which needs a token the browser may no longer
 * hold. Discarding it let the caller navigate to the sign-in page on a 429 with
 * the session cookie neither revoked nor expired, so the next person to open
 * that tab was signed in as the account that thought it had left.
 */
export async function signOut(): Promise<AuthResult<null>> {
	const { error } = await api.POST('/api/auth/logout', {
		headers: csrfHeaders(),
	});
	// 204: there is no body to read, so the absence of a refusal is the answer.
	return error
		? { outcome: 'refused', problem: asProblem(error) }
		: { outcome: 'ok', value: null };
}

/** Reads the signed-in account, if there is one. */
export async function readSession(): Promise<AuthResult<SignedIn>> {
	const { data, error } = await api.GET('/api/auth/session');
	return data
		? { outcome: 'ok', value: data as SignedIn }
		: { outcome: 'refused', problem: asProblem(error) };
}

/** A started Plex sign-in. */
export interface PinStarted {
	id: string;
	code: string;
	authorizationUrl?: string | null;
	expiresAt: number;
}

/** What one poll of a Plex sign-in found. */
export type PinState =
	| { state: 'pending' }
	| {
			state: 'authorized';
			userId: string;
			username: string;
			isAdmin: boolean;
	  }
	| { state: 'expired' };

/** Starts a Plex sign-in, by code or by hosted sign-in. */
export async function startPlexPin(
	oauth: boolean,
): Promise<AuthResult<PinStarted>> {
	const { data, error } = await api.POST('/api/auth/plex/pin', {
		body: { oauth, forwardUrl: oauth ? window.location.href : null },
		headers: csrfHeaders(),
	});
	return data
		? { outcome: 'ok', value: data as PinStarted }
		: { outcome: 'refused', problem: asProblem(error) };
}

/**
 * Asks whether one Plex sign-in has finished.
 *
 * A `POST`, because of what the answer can do: the call that finds the
 * exchange complete consumes it, stores a token, and is handed a session
 * cookie. A `GET` is what a cross-site navigation and a prefetch can reach, and
 * the API's cross-site check exempts every safe method, so the completion has
 * to be a write for the check to apply to it at all.
 */
export async function pollPlexPin(id: string): Promise<AuthResult<PinState>> {
	const { data, error } = await api.POST('/api/auth/plex/pin/{id}', {
		params: { path: { id } },
		headers: csrfHeaders(),
	});
	return data
		? { outcome: 'ok', value: data as PinState }
		: { outcome: 'refused', problem: asProblem(error) };
}
