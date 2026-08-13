// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, test } from 'bun:test';
import {
	loadingTreatment,
	PROGRESS_AFTER_MS,
	SKELETON_AFTER_MS,
} from './loading-policy';
import { EVERY_STATE_KIND, type SurfaceState } from './surface-state';

describe('the loading sub-policy', () => {
	test('shows nothing under 300ms', () => {
		expect(loadingTreatment(0)).toBe('nothing');
		expect(loadingTreatment(SKELETON_AFTER_MS - 1)).toBe('nothing');
	});

	test('shows a skeleton from 300ms', () => {
		expect(loadingTreatment(SKELETON_AFTER_MS)).toBe('skeleton');
		expect(loadingTreatment(PROGRESS_AFTER_MS - 1)).toBe('skeleton');
	});

	test('shows progress beyond three seconds', () => {
		expect(loadingTreatment(PROGRESS_AFTER_MS)).toBe('progress');
		expect(loadingTreatment(60_000)).toBe('progress');
	});

	test('the bands do not overlap or leave a gap', () => {
		const seen = new Set<string>();
		for (let elapsed = 0; elapsed <= 5000; elapsed += 50) {
			seen.add(loadingTreatment(elapsed));
		}
		expect([...seen].sort()).toEqual(['nothing', 'progress', 'skeleton']);
	});
});

describe('the state vocabulary', () => {
	test('all nine states are named', () => {
		expect(EVERY_STATE_KIND.length).toBe(9);
		expect(new Set(EVERY_STATE_KIND).size).toBe(9);
	});

	test('the six engine states are distinct from the three universal ones', () => {
		const universal = ['loading', 'empty', 'error'];
		const engine = EVERY_STATE_KIND.filter((kind) => !universal.includes(kind));
		expect(engine).toEqual([
			'frozen',
			'degraded',
			'stale',
			'pending',
			'blocked',
			'non-convergent',
		]);
	});

	test('every kind can be constructed, so none is unreachable', () => {
		// The compiler proves the shapes; this proves the list and the union
		// agree, which is what `I-UX-1`'s coverage assertion rests on.
		const examples: SurfaceState[] = [
			{ kind: 'loading', elapsedMs: 0 },
			{ kind: 'empty', reason: 'nothingCreated' },
			{ kind: 'error', summary: 'Radarr (4K) did not respond' },
			{ kind: 'frozen', source: 'Trakt', lastSuccessAge: '2 hours' },
			{ kind: 'degraded', capability: 'TMDB ratings' },
			{ kind: 'stale', age: '3 days' },
			{ kind: 'pending', operation: 'Creating a placeholder' },
			{ kind: 'blocked', reason: 'An ambiguous match needs a decision' },
			{ kind: 'non-convergent', unsettled: ['Hub 4'] },
		];
		expect(examples.map((state) => state.kind).sort()).toEqual(
			[...EVERY_STATE_KIND].sort(),
		);
	});

	test('the three empty reasons stay distinct', () => {
		const reasons: SurfaceState[] = [
			{ kind: 'empty', reason: 'nothingCreated' },
			{ kind: 'empty', reason: 'nothingMatched', predicate: 'year >= 2020' },
			{ kind: 'empty', reason: 'pending' },
		];
		const distinct = new Set(
			reasons.map((state) => (state.kind === 'empty' ? state.reason : '')),
		);
		expect(distinct.size).toBe(3);
	});
});
