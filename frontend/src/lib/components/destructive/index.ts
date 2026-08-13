// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * The affordance every destructive action wears.
 *
 * Preview with named counts, confirmation proportional to consequence, and a
 * report afterwards — never a default, focused, or single-step destructive
 * action (PRD §8.5).
 */

import DestructiveAction from './destructive-action.svelte';

export type { Consequence } from './consequence';
export { confirmationSatisfied } from './consequence';
export { DestructiveAction };
