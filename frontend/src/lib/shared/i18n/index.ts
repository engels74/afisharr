// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * The message catalogue.
 *
 * Everything user-visible in this interface resolves through {@link t} or
 * {@link tn}, from the first commit (`I-UX-7`). `scripts/lint-interface.ts`
 * fails the build on a hard-coded string, so the habit is enforced rather than
 * remembered.
 */

export { formatDuration } from './duration';
export type { Values } from './interpolate';
export { interpolate, placeholdersOf } from './interpolate';
export type { Catalogue, MessageKey, PluralKey } from './messages.svelte';
export { english, locale, t, tn, useCatalogue } from './messages.svelte';
export type { PluralCategory, PluralForms } from './plural';
export { selectPlural } from './plural';
