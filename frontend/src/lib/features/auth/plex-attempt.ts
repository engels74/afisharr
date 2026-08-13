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

/**
 * The attempt this tab left behind, if it is still worth polling.
 *
 * The storage access is inside the `try`, not beside it. Touching
 * `sessionStorage` *throws* `SecurityError` rather than answering `null` when
 * site data is blocked for the origin, or when the document is a cross-origin
 * frame that has not been granted storage access — and an Afisharr embedded in
 * a dashboard iframe is an ordinary deployment for this class of app. This runs
 * from a `$state` initialiser during component setup, so the exception took the
 * whole sign-in page down with it: the operator could not reach the
 * username-and-password form either, on a page that renders as an error with no
 * way to sign in at all. Resuming a parked attempt is the only thing storage is
 * needed for, so a browser that refuses it loses exactly that.
 */
export function resumeAttempt(): PinStarted | undefined {
	try {
		const stored = sessionStorage.getItem(RESUME_KEY);
		// Read once: a pin that was not polled to a conclusion this time is not
		// worth resuming on the load after either.
		sessionStorage.removeItem(RESUME_KEY);
		if (!stored) {
			return undefined;
		}
		const attempt = JSON.parse(stored) as PinStarted;
		return attempt.expiresAt > Date.now() ? attempt : undefined;
	} catch {
		return undefined;
	}
}

/**
 * Parks `attempt` for the document plex.tv returns the operator to.
 *
 * Best effort, for the reason {@link resumeAttempt} gives: a browser that
 * refuses storage refuses it here too, and a hosted sign-in that cannot be
 * resumed is better than a start button that throws.
 */
export function parkAttempt(attempt: PinStarted): void {
	try {
		sessionStorage.setItem(RESUME_KEY, JSON.stringify(attempt));
	} catch {
		// Nothing to park it in. The exchange still runs in this document.
	}
}

/** Drops a parked attempt that will never be polled to a conclusion. */
export function forgetAttempt(): void {
	try {
		sessionStorage.removeItem(RESUME_KEY);
	} catch {
		// Nothing was parked, because nothing could be.
	}
}

/** Whether `problem` is the instance refusing the return target itself. */
export function refusedTheReturnTarget(problem: Problem): boolean {
	return problem.pointer === '/forwardUrl';
}
