// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * What the operator can choose, and what `mode-watcher` persists.
 *
 * Three, not two. A control that only toggles light against dark takes the
 * "follow the system" default away the first time it is pressed and offers no
 * way back to it, which is a choice the operator did not make and cannot undo
 * from the interface (P2).
 */
export type ModeChoice = 'system' | 'light' | 'dark';

/** The choices, in the order the control offers them. */
export const MODE_CHOICES = [
	'system',
	'light',
	'dark',
] as const satisfies readonly ModeChoice[];

/** Whether `value` is one of the three. */
export function isModeChoice(value: string): value is ModeChoice {
	return (MODE_CHOICES as readonly string[]).includes(value);
}

/**
 * What a browser has to expose before its color-scheme preference is readable.
 *
 * Typed as the one property that is asked about rather than as `Window`,
 * because the interesting case is a browser where it is missing.
 */
export interface PreferenceReader {
	readonly matchMedia?: unknown;
}

/**
 * The mode to set explicitly, or `undefined` where the system can be asked.
 *
 * `mode-watcher` resolves the system preference by testing
 * `(prefers-color-scheme: light)` and maps every non-match to dark — including
 * a browser with no `window.matchMedia` at all, where nothing was tested and
 * nothing was learned. A failed observation is not an observation of dark (P1),
 * and PRD §10.4 fixes the answer for that case: light. A dark interface shown
 * to somebody who asked for neither is the miss that reads as broken.
 *
 * Answers `undefined` — not `'system'` — where the query can run, because the
 * two are different acts: one sets a stored preference, and the other leaves
 * the operator's alone so a genuine system-dark preference is still honoured.
 */
export function fallbackMode(
	view: PreferenceReader | undefined,
): 'light' | undefined {
	return typeof view?.matchMedia === 'function' ? undefined : 'light';
}
