// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, test } from 'bun:test';
import { en, enPlurals } from './catalogue.en';
import { interpolate, placeholdersOf } from './interpolate';
import { english, locale, t, tn, useCatalogue } from './messages.svelte';
import { selectPlural } from './plural';

describe('the English catalogue', () => {
	test('every message is a non-empty string', () => {
		for (const [key, value] of Object.entries(en)) {
			expect(value.length, `${key} is empty`).toBeGreaterThan(0);
		}
	});

	test('no message is a bare placeholder with nothing around it', () => {
		// `'{count}'` as a whole message is a key that should have been a
		// plural form or a formatter, not a catalogue entry.
		for (const [key, value] of Object.entries(en)) {
			expect(value.trim(), `${key} is only a placeholder`).not.toMatch(
				/^\{[a-zA-Z][a-zA-Z0-9_]*\}$/,
			);
		}
	});

	test('every plural entry supplies the category every language has', () => {
		for (const [key, forms] of Object.entries(enPlurals)) {
			expect(forms.other, `${key} has no "other" form`).toBeTruthy();
		}
	});

	test('every plural form fills the same placeholders', () => {
		for (const [key, forms] of Object.entries(enPlurals)) {
			const expected = placeholdersOf(forms.other).sort();
			for (const [category, form] of Object.entries(forms)) {
				expect(
					placeholdersOf(form as string).sort(),
					`${key}.${category} fills different placeholders`,
				).toEqual(expected);
			}
		}
	});

	test('English ships as the active catalogue', () => {
		expect(english.locale).toBe('en');
		expect(Object.keys(english.messages).length).toBe(Object.keys(en).length);
	});
});

describe('interpolation', () => {
	test('a placeholder is filled from the values', () => {
		expect(interpolate('Hello {name}', { name: 'operator' })).toBe(
			'Hello operator',
		);
	});

	test('a number is rendered rather than dropped', () => {
		expect(interpolate('{count} left', { count: 0 })).toBe('0 left');
	});

	test('a placeholder with no value is left as written', () => {
		// `undefined items` reads as an engine fault; `{count} items` reads as
		// a catalogue fault, which is what it is.
		expect(interpolate('{count} items')).toBe('{count} items');
	});

	test('text with no placeholders is returned unchanged', () => {
		expect(interpolate('Sign in')).toBe('Sign in');
	});

	test('a brace that is not an identifier is not a placeholder', () => {
		expect(interpolate('{ } and {1}')).toBe('{ } and {1}');
	});
});

describe('plural selection', () => {
	test('English picks "one" at one and "other" everywhere else', () => {
		const forms = { one: 'one thing', other: 'many things' };
		expect(selectPlural('en', 1, forms)).toBe('one thing');
		expect(selectPlural('en', 0, forms)).toBe('many things');
		expect(selectPlural('en', 2, forms)).toBe('many things');
	});

	test('a language whose category the entry omits falls back to "other"', () => {
		// Polish uses `few` at 2; an English-shaped entry has no `few`, and the
		// fallback keeps the sentence readable rather than undefined.
		expect(selectPlural('pl', 2, { one: 'jeden', other: 'wiele' })).toBe(
			'wiele',
		);
	});
});

describe('the catalogue in force', () => {
	test('English is the one that ships', () => {
		expect(locale()).toBe('en');
	});

	test('a replacement catalogue takes effect for both lookups', () => {
		// The shape a second language arrives in. Swapping it must not need a
		// second lookup function, or the two would drift.
		useCatalogue({
			locale: 'en-GB',
			messages: { ...en, 'auth.signIn': 'Sign in, please' },
			plurals: enPlurals,
		});
		expect(locale()).toBe('en-GB');
		expect(t('auth.signIn')).toBe('Sign in, please');
		expect(tn('count.items', 2)).toBe('2 items');

		useCatalogue(english);
		expect(t('auth.signIn')).toBe('Sign in');
	});
});

describe('lookup', () => {
	test('a flat message resolves', () => {
		expect(t('auth.signIn')).toBe('Sign in');
	});

	test('a flat message with a placeholder resolves and is filled', () => {
		expect(t('setup.step', { ordinal: 3 })).toBe('Step 3 of 8');
	});

	test('a counted message fills count without being passed it twice', () => {
		expect(tn('count.items', 1)).toBe('1 item');
		expect(tn('count.items', 4)).toBe('4 items');
		expect(tn('count.items', 0)).toBe('0 items');
	});
});
