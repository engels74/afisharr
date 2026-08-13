// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/** How the interface decides between its two modes, and how the operator overrides that. */

import ModePreference from './mode-preference.svelte';
import ModeToggle from './mode-toggle.svelte';

export type { ModeChoice, PreferenceReader } from './system-mode';
export { fallbackMode, isModeChoice, MODE_CHOICES } from './system-mode';
export { ModePreference, ModeToggle };
