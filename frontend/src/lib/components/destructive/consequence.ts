// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * How much a destructive action costs if it turns out to be wrong.
 *
 * Two levels, because PRD §8.5 names two: a button for actions that pressing
 * again undoes, and a typed confirmation for teardown, full hub reset, and bulk
 * placeholder removal.
 */
export type Consequence = 'reversible' | 'typed';

/**
 * Whether the confirmation for `consequence` has been satisfied.
 *
 * A typed confirmation compares exactly, after trimming: an operator who typed
 * a trailing space has typed the phrase, and one who typed something else has
 * not. Case is not folded — the phrase is shown, and matching it is the point.
 */
export function confirmationSatisfied(
	consequence: Consequence,
	phrase: string,
	typed: string,
): boolean {
	if (consequence === 'reversible') {
		return true;
	}
	return phrase.length > 0 && typed.trim() === phrase;
}
