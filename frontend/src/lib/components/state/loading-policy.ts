// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/** What a loading surface should show right now. */
export type LoadingTreatment = 'nothing' | 'skeleton' | 'progress';

/** Under this, show nothing: a flash of skeleton is worse than a brief pause. */
export const SKELETON_AFTER_MS = 300;

/** Beyond this, say what is happening. */
export const PROGRESS_AFTER_MS = 3000;

/**
 * The loading sub-policy from PRD §8.2, as one function.
 *
 * A spinner that has been turning for eight seconds is indistinguishable from
 * a hang, which is why the third band exists and why it carries text rather
 * than a longer animation.
 */
export function loadingTreatment(elapsedMs: number): LoadingTreatment {
	if (elapsedMs < SKELETON_AFTER_MS) {
		return 'nothing';
	}
	if (elapsedMs < PROGRESS_AFTER_MS) {
		return 'skeleton';
	}
	return 'progress';
}
