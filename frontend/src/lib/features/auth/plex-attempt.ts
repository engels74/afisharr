// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * The lifecycle of one plex.tv pin attempt, apart from the panel that shows it.
 *
 * None of this is rendering: how often to ask, where an attempt is parked
 * across the hosted sign-in, and how to tell the instance refusing the return
 * target from any other refusal. It is split out because the panel is a view
 * and these are the rules the view obeys.
 */

import type { Problem } from '$lib/api/client';

import type { PinStarted } from './auth-client';

/**
 * How often to ask plex.tv whether the operator has finished.
 *
 * Chosen against the budget it spends, not against how fast a code can be
 * typed. Every poll reaches plex.tv, so every poll costs one of the sixty
 * provider attempts an address gets each minute — and `trustProxy` is empty
 * by default, so behind the reverse proxy nearly every deployment runs,
 * every caller resolves to the proxy's one address and shares that counter.
 * At two seconds a single panel spent thirty of the sixty, so two operators
 * signing in at once — or one operator with the page open in two tabs —
 * refused each other. At five it is twelve, which leaves room for five
 * concurrent sign-ins and still notices a finished exchange within a few
 * seconds of the operator finishing it (PRD §21.4.3).
 */
export const POLL_INTERVAL_MS = 5000;

/**
 * Where an in-flight attempt is kept across the hosted sign-in.
 *
 * The OAuth variant leaves the page by top-level navigation and returns to
 * a fresh document, so an attempt held only in component state is an
 * attempt nobody polls: plex.tv has authorised a pin this build has
 * forgotten, and the operator's only move is to start another one. Session
 * storage rather than local: it belongs to the tab that started it and has
 * no business outliving it.
 */
const RESUME_KEY = 'afisharr.plexAttempt';

/** The attempt this tab left behind, if it is still worth polling. */
export function resumeAttempt(): PinStarted | undefined {
	const stored = sessionStorage.getItem(RESUME_KEY);
	// Read once: a pin that was not polled to a conclusion this time is not
	// worth resuming on the load after either.
	sessionStorage.removeItem(RESUME_KEY);
	if (!stored) {
		return undefined;
	}
	try {
		const attempt = JSON.parse(stored) as PinStarted;
		return attempt.expiresAt > Date.now() ? attempt : undefined;
	} catch {
		return undefined;
	}
}

/** Parks `attempt` for the document plex.tv returns the operator to. */
export function parkAttempt(attempt: PinStarted): void {
	sessionStorage.setItem(RESUME_KEY, JSON.stringify(attempt));
}

/** Drops a parked attempt that will never be polled to a conclusion. */
export function forgetAttempt(): void {
	sessionStorage.removeItem(RESUME_KEY);
}

/** Whether `problem` is the instance refusing the return target itself. */
export function refusedTheReturnTarget(problem: Problem): boolean {
	return problem.pointer === '/forwardUrl';
}
