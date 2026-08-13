// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/** The values a message's placeholders are filled from. */
export type Values = Record<string, string | number>;

/** Matches `{name}`, and nothing that is not a bare identifier. */
const PLACEHOLDER = /\{([a-zA-Z][a-zA-Z0-9_]*)\}/g;

/**
 * Fills `{name}` placeholders in `template` from `values`.
 *
 * A placeholder with no value is left as it was written rather than replaced
 * with `undefined`. An operator who sees `{count}` on screen has been told
 * something is wrong with the catalogue; one who sees `undefined items` has
 * been told the engine is broken, which is the wrong thing to have learned.
 */
export function interpolate(template: string, values: Values = {}): string {
	return template.replace(PLACEHOLDER, (whole, name: string) => {
		const value = values[name];
		return value === undefined ? whole : String(value);
	});
}

/**
 * Every placeholder `template` uses, in the order they appear.
 *
 * Used by the catalogue's own test to assert that a translation fills the same
 * placeholders as the English it replaces.
 */
export function placeholdersOf(template: string): string[] {
	return [...template.matchAll(PLACEHOLDER)].map((match) => match[1]);
}
