// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * Where this instance's source lives.
 *
 * AGPL-3.0-or-later section 13 obliges any modified instance offered to other
 * users over a network to make its source available to them, so the interface
 * carries a permanent link reachable from every page (D-028, PRD §6.4). Two
 * things follow, and both are easy to get wrong:
 *
 * 1. It resolves to the **running version**, not to whatever is on the default
 *    branch. A link to `main` from a six-month-old container satisfies nobody.
 * 2. It **survives forking**. Someone running a modified build inherits the
 *    same obligation, so the target is configurable rather than compiled in.
 */

/** Where the unmodified project lives. A fork replaces this. */
const DEFAULT_REPOSITORY = 'https://github.com/engels74/afisharr';

/** What the interface links to when it does not yet know the version. */
const UNVERSIONED = DEFAULT_REPOSITORY;

/** The version and repository this build reports. */
export interface Provenance {
	/** The running binary's version, from the health route. */
	readonly version?: string;
	/** The repository this build's source is published at. */
	readonly repository?: string;
}

let current: Provenance = {};

/**
 * Records what the instance reported about itself.
 *
 * Called once, from the health response. Before that the link still resolves —
 * to the repository root — because a footer that renders nothing until a fetch
 * lands is a licence obligation with a loading state.
 */
export function recordProvenance(provenance: Provenance): void {
	current = provenance;
}

/** The link the footer renders. */
export function sourceHref(): string {
	const repository = current.repository ?? DEFAULT_REPOSITORY;
	if (!current.version) {
		return UNVERSIONED;
	}
	return `${repository.replace(/\/+$/, '')}/tree/v${current.version}`;
}
