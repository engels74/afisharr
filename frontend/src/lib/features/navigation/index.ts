// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/** The navigation shell: six primary destinations plus a settings area. */

import NavShell from './nav-shell.svelte';

export type { Destination } from './destinations';
export {
	isActive,
	isBareRoute,
	LOGIN,
	landingFor,
	PRIMARY,
	SETTINGS,
	SETTINGS_SUBPAGES,
	SETUP,
} from './destinations';
export { NavShell };
