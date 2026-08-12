// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { tn } from './messages.svelte';

/** The point at which a wait reads better in minutes than in seconds. */
const A_MINUTE = 60;

/**
 * A wait, in the words of the catalogue in force.
 *
 * Here rather than at each call site because `retryAfterSeconds` is a number
 * the API returns and every surface that shows one has the same two problems
 * with it. Written inline as `` `${seconds}s` `` it was English typed into a
 * component — the exact thing `I-UX-7` forbids and the exact thing
 * `scripts/lint-interface.ts` cannot see, because that rule reads markup and
 * this sat in a `<script>` block. It was also unreadable: a fifteen-minute
 * lockout rendered as `900s`, which an operator has to divide before it means
 * anything.
 *
 * Rounded *up*, both ways. The number is "you may try again after this", so a
 * value rounded down invites a retry that is refused again, and a sub-second
 * wait still has to say something.
 */
export function formatDuration(seconds: number): string {
	if (seconds >= A_MINUTE) {
		return tn('count.minutes', Math.ceil(seconds / A_MINUTE));
	}
	return tn('count.seconds', Math.max(1, Math.ceil(seconds)));
}
