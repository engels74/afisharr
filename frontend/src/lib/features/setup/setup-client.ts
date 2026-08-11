// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { api, asProblem, csrfHeaders, type Problem } from '$lib/api/client';

/** Where the wizard is, as the server derives it. */
export interface SetupStatus {
	step:
		| 'claim'
		| 'admin'
		| 'plex'
		| 'libraries'
		| 'integrations'
		| 'packs'
		| 'report'
		| 'review';
	ordinal: number;
	claimHeld: boolean;
	recoveryAvailable: boolean;
	tokenLive: boolean;
}

/**
 * What the claim page renders, before it holds a claim.
 *
 * A separate shape from {@link SetupStatus} because it comes from a separate
 * route, and it has to: `/api/setup/status` sits behind the claim gate, so an
 * unclaimed browser is always refused there. A page that filled these in
 * itself would hide the recovery form from the operator whose token died with
 * a restart, and offer a token field on an instance with no live token.
 */
export interface ClaimStatus {
	ordinal: number;
	claimHeld: boolean;
	recoveryAvailable: boolean;
	tokenLive: boolean;
}

/** What a setup call produced. Never an exception: a refusal is a value. */
export type SetupResult<T> =
	| { readonly outcome: 'ok'; readonly value: T }
	| { readonly outcome: 'refused'; readonly problem: Problem };

/**
 * Reads the derived step.
 *
 * There is no step parameter to pass, and that is the design: a step index in
 * a query string would let a caller name the step they would like to be on,
 * which on the claim step means naming step 2 (D-046, `I-UX-10`).
 */
export async function readStatus(): Promise<SetupResult<SetupStatus>> {
	const { data, error } = await api.GET('/api/setup/status');
	return data
		? { outcome: 'ok', value: data as SetupStatus }
		: { outcome: 'refused', problem: asProblem(error) };
}

/**
 * Reads what the claim page renders, without needing a claim.
 *
 * The one setup read available before the gate is passed. Everything it
 * returns is a fact the server derived, so the page never invents one.
 */
export async function readClaimStatus(): Promise<SetupResult<ClaimStatus>> {
	const { data, error } = await api.GET('/api/setup/claim');
	return data
		? { outcome: 'ok', value: data as ClaimStatus }
		: { outcome: 'refused', problem: asProblem(error) };
}

/** Claims the wizard with the token printed on the console. */
export async function claim(
	token: string,
): Promise<SetupResult<{ expiresAt: number }>> {
	const { data, error } = await api.POST('/api/setup/claim', {
		body: { token },
		headers: csrfHeaders(),
	});
	return data
		? { outcome: 'ok', value: data }
		: { outcome: 'refused', problem: asProblem(error) };
}

/** Claims the wizard with administrator credentials, once one exists. */
export async function recover(
	username: string,
	password: string,
): Promise<SetupResult<{ expiresAt: number }>> {
	const { data, error } = await api.POST('/api/setup/recover', {
		body: { username, password },
		headers: csrfHeaders(),
	});
	return data
		? { outcome: 'ok', value: data }
		: { outcome: 'refused', problem: asProblem(error) };
}

/** Creates the first-run administrator. */
export async function createAdmin(
	username: string,
	password: string,
): Promise<SetupResult<{ userId: string; username: string }>> {
	const { data, error } = await api.POST('/api/setup/admin', {
		body: { username, password },
		headers: csrfHeaders(),
	});
	return data
		? { outcome: 'ok', value: data }
		: { outcome: 'refused', problem: asProblem(error) };
}

/**
 * Finishes setup.
 *
 * Creating the administrator does not finish setup — it only moves the derived
 * step on. Until this is called `instance.setup_completed_at` is `NULL`, and
 * `require_setup_completed` refuses every route behind it, sign-in included.
 * A first run that stopped after the administrator would leave the shell
 * unreachable through the interface that created it.
 */
export async function completeSetup(): Promise<SetupResult<SetupStatus>> {
	const { data, error } = await api.POST('/api/setup/complete', {
		headers: csrfHeaders(),
	});
	return data
		? { outcome: 'ok', value: data as SetupStatus }
		: { outcome: 'refused', problem: asProblem(error) };
}
