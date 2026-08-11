// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { en, enPlurals } from './catalogue.en';
import { interpolate, type Values } from './interpolate';
import { type PluralForms, selectPlural } from './plural';

/** Every key the interface may ask for. Derived, so a typo is a type error. */
export type MessageKey = keyof typeof en;

/** Every key whose text depends on a count. */
export type PluralKey = keyof typeof enPlurals;

/** A catalogue: the flat messages and the counted ones, for one language. */
export interface Catalogue {
	/** The BCP 47 tag this catalogue is written in. */
	readonly locale: string;
	/** The flat messages. */
	readonly messages: Record<MessageKey, string>;
	/**
	 * The counted messages.
	 *
	 * Typed as {@link PluralForms} rather than as English's own strings: the
	 * key set is fixed and the text is not. A second catalogue has to be able
	 * to say something different, in a language whose plural rules may need a
	 * category English has never had.
	 */
	readonly plurals: Record<PluralKey, PluralForms>;
}

/** English, which every build ships complete. */
export const english: Catalogue = {
	locale: 'en',
	messages: en,
	plurals: enPlurals,
};

/**
 * The catalogue in force.
 *
 * `$state.raw` rather than `$state`: a catalogue is replaced wholesale when the
 * locale changes and is never mutated in place, so the deep proxy would cost a
 * wrap of every message for a write that never happens.
 */
let active = $state.raw<Catalogue>(english);

/** Replaces the catalogue in force. */
export function useCatalogue(catalogue: Catalogue): void {
	active = catalogue;
}

/** The locale the interface is currently formatting in. */
export function locale(): string {
	return active.locale;
}

/**
 * The message for `key`, with `{name}` placeholders filled from `values`.
 *
 * Every user-facing string in the interface goes through here (`I-UX-7`). The
 * key is a literal at every call site — never assembled from a variable —
 * because a catalogue you cannot grep is a catalogue that quietly grows dead
 * entries and quietly loses live ones.
 */
export function t(key: MessageKey, values?: Values): string {
	return interpolate(active.messages[key], values);
}

/** The message for `key` at `count`, with `{count}` already filled. */
export function tn(key: PluralKey, count: number, values?: Values): string {
	const form = selectPlural(active.locale, count, active.plurals[key]);
	return interpolate(form, { count, ...values });
}
