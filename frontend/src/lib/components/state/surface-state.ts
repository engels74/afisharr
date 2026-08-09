// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * The nine states, as a discriminated union.
 *
 * Three are universal to any interface; six are specific to what the engine
 * knows, and those six are the ones that make the product honest (PRD §8.1).
 * They arrive as an explicit field on the response and are never derived from
 * response shape, timing, or an empty array (`I-UX-2`) — which is why every
 * variant below carries the facts the treatment needs, rather than the
 * component inferring them.
 */
export type SurfaceState =
	| Loading
	| Empty
	| SurfaceError
	| Frozen
	| Degraded
	| Stale
	| Pending
	| Blocked
	| NonConvergent;

/** Data is being fetched. */
export interface Loading {
	readonly kind: 'loading';
	/** Milliseconds since the fetch began, so the sub-policy can be applied. */
	readonly elapsedMs: number;
	/** What is happening, for the beyond-three-seconds treatment. */
	readonly progress?: string;
}

/** Successfully retrieved, nothing to show. */
export interface Empty {
	readonly kind: 'empty';
	/** Which of the three kinds this is. Conflating them is the common failure. */
	readonly reason: EmptyReason;
	/** The narrowing predicate, for `nothingMatched`. */
	readonly predicate?: string;
}

/**
 * Why a surface is empty.
 *
 * `nothingMatched` is never shown for a failed fetch: that conflation is the
 * interface expression of P1, and it teaches the operator to distrust every
 * empty state in the product (PRD §8.3).
 */
export type EmptyReason = 'nothingCreated' | 'nothingMatched' | 'pending';

/** The request failed. */
export interface SurfaceError {
	readonly kind: 'error';
	/** What failed, in terms the operator recognises. */
	readonly summary: string;
	/** What it means for them. */
	readonly consequence?: string;
	/** The technical detail, kept available and collapsed. */
	readonly detail?: string;
}

/** A source failed; the contribution is held at last-known-good. */
export interface Frozen {
	readonly kind: 'frozen';
	/** The source, named the way the operator configured it. */
	readonly source: string;
	/** How long ago it last succeeded, already formatted. */
	readonly lastSuccessAge: string;
}

/** Working, but a capability is unavailable. */
export interface Degraded {
	readonly kind: 'degraded';
	/** The capability that is missing. */
	readonly capability: string;
	/** Where to go to supply it. */
	readonly configureHref?: string;
}

/** Evidence could not be refreshed; state is being preserved deliberately. */
export interface Stale {
	readonly kind: 'stale';
	/** How old the evidence is, already formatted. */
	readonly age: string;
}

/** An intent is committed but not yet confirmed. */
export interface Pending {
	readonly kind: 'pending';
	/** What is in flight. */
	readonly operation: string;
}

/** Action is refused pending a human decision. */
export interface Blocked {
	readonly kind: 'blocked';
	/** What is blocked, and why. */
	readonly reason: string;
	/** When it can be tried again, already formatted. */
	readonly retryAfter?: string;
	/** The one action that unblocks it. */
	readonly unblockLabel?: string;
}

/** An ordering surface could not be settled within the escalation ladder. */
export interface NonConvergent {
	readonly kind: 'non-convergent';
	/** The specific items that would not settle. */
	readonly unsettled: readonly string[];
}

/** Every state's discriminant, for the coverage assertion `I-UX-1` needs. */
export const EVERY_STATE_KIND = [
	'loading',
	'empty',
	'error',
	'frozen',
	'degraded',
	'stale',
	'pending',
	'blocked',
	'non-convergent',
] as const satisfies readonly SurfaceState['kind'][];
