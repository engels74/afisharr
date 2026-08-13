// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * One component per interface state, each independently importable.
 *
 * The engine distinguishes nine states so the interface can tell the truth
 * about what it knows (PRD §8.1). Having one component each is what stops a
 * page inventing its own flattening: a surface that needs "frozen" imports the
 * frozen treatment rather than reaching for the error one because it is nearer.
 */

import BlockedState from './blocked-state.svelte';
import DegradedState from './degraded-state.svelte';
import EmptyState from './empty-state.svelte';
import ErrorState from './error-state.svelte';
import FrozenState from './frozen-state.svelte';
import LoadingState from './loading-state.svelte';
import NonConvergentState from './non-convergent-state.svelte';
import PendingState from './pending-state.svelte';
import StaleState from './stale-state.svelte';

export type { LoadingTreatment } from './loading-policy';
export {
	loadingTreatment,
	PROGRESS_AFTER_MS,
	SKELETON_AFTER_MS,
} from './loading-policy';
export type {
	Blocked,
	Degraded,
	Empty,
	EmptyReason,
	Frozen,
	Loading,
	NonConvergent,
	Pending,
	Stale,
	SurfaceError,
	SurfaceState,
} from './surface-state';
export { EVERY_STATE_KIND } from './surface-state';
export {
	BlockedState,
	DegradedState,
	EmptyState,
	ErrorState,
	FrozenState,
	LoadingState,
	NonConvergentState,
	PendingState,
	StaleState,
};
