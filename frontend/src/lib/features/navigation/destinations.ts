// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import type { MessageKey } from '$lib/shared/i18n';

/** One entry in the navigation shell. */
export interface Destination {
	/** Where it goes. */
	readonly href: string;
	/** Its label's catalogue key. Never the label itself (`I-UX-7`). */
	readonly label: MessageKey;
}

/**
 * The six primary destinations, in the order PRD §6.1 fixes them.
 *
 * Organised around the object the operator is thinking about, not the
 * subsystem that owns it. Two of the six are deliberate: Doctor is primary
 * navigation because it is where the product's honesty becomes visible, and
 * Lifecycle is primary because it is the differentiator.
 */
export const PRIMARY: readonly Destination[] = [
	{ href: '/dashboard', label: 'nav.dashboard' },
	{ href: '/collections', label: 'nav.collections' },
	{ href: '/design', label: 'nav.design' },
	{ href: '/home-screen', label: 'nav.homeScreen' },
	{ href: '/lifecycle', label: 'nav.lifecycle' },
	{ href: '/doctor', label: 'nav.doctor' },
];

/** The settings area, which sits apart from the six. */
export const SETTINGS: Destination = {
	href: '/settings',
	label: 'nav.settings',
};

/** The settings sub-pages, in the order §7.13 lists them. */
export const SETTINGS_SUBPAGES: readonly Destination[] = [
	{ href: '/settings/plex', label: 'page.settings.plex' },
	{ href: '/settings/integrations', label: 'page.settings.integrations' },
	{ href: '/settings/libraries', label: 'page.settings.libraries' },
	{ href: '/settings/users', label: 'page.settings.users' },
	{ href: '/settings/general', label: 'page.settings.general' },
	{ href: '/settings/teardown', label: 'page.settings.teardown' },
	{ href: '/settings/about', label: 'page.settings.about' },
];

/**
 * Whether `pathname` is inside `destination`.
 *
 * Prefix matching on a path segment, so `/settings/plex` marks Settings active
 * and `/collections-archive` does not mark Collections active.
 */
export function isActive(destination: Destination, pathname: string): boolean {
	return (
		pathname === destination.href || pathname.startsWith(`${destination.href}/`)
	);
}
