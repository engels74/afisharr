// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * How long to wait for a heartbeat before saying the stream is gone.
 *
 * `I-UX-9` asks for the indicator within one missed heartbeat. The next beat is
 * due one interval after the last event, so a fifth of an interval of grace
 * puts the answer just past the moment one has actually been missed — 18
 * seconds at the server's 15 — rather than at some constant that has nothing to
 * do with what the server said. The absolute floor only matters at the very
 * short intervals a test uses.
 */
export function watchdogDelayMs(heartbeatMs: number): number {
	return Math.max(heartbeatMs * 1.2, heartbeatMs + 250);
}

/** What the stream is doing, as the indicator renders it. */
export type StreamStatus =
	| 'connecting'
	| 'live'
	| 'reconnecting'
	| 'disconnected';

/** The first retry delay. */
export const BASE_DELAY_MS = 1000;

/** The longest a retry ever waits. */
export const MAX_DELAY_MS = 30_000;

/**
 * How long to wait before retry `attempt`, one-based.
 *
 * Exponential and capped, with jitter. The cap keeps a browser tab left open
 * overnight from waiting an hour to notice the instance came back; the jitter
 * keeps four tabs on one machine from reconnecting in lockstep and arriving as
 * a burst every time the container restarts.
 */
export function backoffDelayMs(
	attempt: number,
	random: () => number = Math.random,
): number {
	const exponent = Math.max(0, attempt - 1);
	const capped = Math.min(BASE_DELAY_MS * 2 ** exponent, MAX_DELAY_MS);
	// Full jitter over the lower half, so the delay is never zero and never
	// longer than the band it belongs to.
	return Math.round(capped / 2 + random() * (capped / 2));
}
