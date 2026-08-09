// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * The plural categories CLDR defines. A catalogue entry supplies the ones its
 * language uses; English uses `one` and `other`, and a language that needs
 * `few` supplies `few` without any code changing.
 */
export type PluralCategory = Intl.LDMLPluralRule;

/** One message that reads differently depending on a count. */
export type PluralForms = Partial<Record<PluralCategory, string>> & {
	/** Every language has this category, so it is the one that is required. */
	other: string;
};

/**
 * Picks the form for `count` in `locale`.
 *
 * `Intl.PluralRules` decides the category, not a hand-written `count === 1`.
 * The hand-written version is right for English and wrong for most of the
 * languages a catalogue will eventually be written in, and it is wrong in a way
 * that only shows up once somebody has translated fifteen pages.
 */
export function selectPlural(
	locale: string,
	count: number,
	forms: PluralForms,
): string {
	const category = new Intl.PluralRules(locale).select(count);
	return forms[category] ?? forms.other;
}
