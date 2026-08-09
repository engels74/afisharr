// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

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
