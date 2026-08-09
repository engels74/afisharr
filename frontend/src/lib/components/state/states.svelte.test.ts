// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';
import BlockedState from './blocked-state.svelte';
import DegradedState from './degraded-state.svelte';
import EmptyState from './empty-state.svelte';
import ErrorState from './error-state.svelte';
import FrozenState from './frozen-state.svelte';
import LoadingState from './loading-state.svelte';
import NonConvergentState from './non-convergent-state.svelte';
import PendingState from './pending-state.svelte';
import StaleState from './stale-state.svelte';
import { EVERY_STATE_KIND } from './surface-state';

/**
 * `I-UX-1` — every state that can reach a component has a treatment.
 *
 * The coverage assertion the invariant asks for: one component per state, each
 * rendering something a person can read. A state with no component fails here
 * rather than arriving at a page that renders it as one of the generic three.
 */
describe('the nine-state vocabulary', () => {
	test('there is one component per state', () => {
		const components = {
			loading: LoadingState,
			empty: EmptyState,
			error: ErrorState,
			frozen: FrozenState,
			degraded: DegradedState,
			stale: StaleState,
			pending: PendingState,
			blocked: BlockedState,
			'non-convergent': NonConvergentState,
		};
		expect(Object.keys(components).sort()).toEqual(
			[...EVERY_STATE_KIND].sort(),
		);
	});
});

describe('loading', () => {
	test('renders nothing under 300ms', async () => {
		const screen = await render(LoadingState, {
			state: { kind: 'loading', elapsedMs: 100 },
		});
		expect(
			screen.container.querySelector('[data-slot="loading-state"]'),
		).toBeNull();
	});

	test('renders a skeleton between 300ms and three seconds', async () => {
		const screen = await render(LoadingState, {
			state: { kind: 'loading', elapsedMs: 1000 },
		});
		const slot = screen.container.querySelector('[data-slot="loading-state"]');
		expect(slot?.getAttribute('data-treatment')).toBe('skeleton');
	});

	test('renders progress text beyond three seconds', async () => {
		const screen = await render(LoadingState, {
			state: {
				kind: 'loading',
				elapsedMs: 5000,
				progress: 'Fetching from TMDB (2 of 5)',
			},
		});
		await expect
			.element(screen.getByText('Fetching from TMDB (2 of 5)'))
			.toBeVisible();
	});
});

describe('empty', () => {
	test('a failed fetch and an empty result render differently', async () => {
		// `I-UX-3`: the interface expression of P1. These two must not look the
		// same, and the failure case must name the failure.
		const empty = await render(EmptyState, {
			state: {
				kind: 'empty',
				reason: 'nothingMatched',
				predicate: 'year >= 2020',
			},
			explanation: 'The filter excluded everything.',
		});
		const failed = await render(ErrorState, {
			state: { kind: 'error', summary: 'Radarr (4K) did not respond' },
		});

		await expect.element(empty.getByText('Nothing matched')).toBeVisible();
		await expect
			.element(failed.getByText('Radarr (4K) did not respond'))
			.toBeVisible();
		expect(empty.container.innerHTML).not.toBe(failed.container.innerHTML);
	});

	test('the three empty kinds carry distinct treatments', async () => {
		const rendered: (string | null | undefined)[] = [];
		for (const reason of [
			'nothingCreated',
			'nothingMatched',
			'pending',
		] as const) {
			const screen = await render(EmptyState, {
				state: { kind: 'empty', reason },
				explanation: 'Explanation',
			});
			rendered.push(
				screen.container
					.querySelector('[data-slot="empty-state"]')
					?.getAttribute('data-reason'),
			);
		}
		expect(rendered).toEqual(['nothingCreated', 'nothingMatched', 'pending']);
	});
});

describe('the six engine states', () => {
	test('frozen names the source and when it last worked, and is not an error', async () => {
		const screen = await render(FrozenState, {
			state: { kind: 'frozen', source: 'Trakt', lastSuccessAge: '2 hours' },
		});
		await expect.element(screen.getByText(/Trakt/)).toBeVisible();
		expect(screen.container.querySelector('[role="alert"]')).toBeNull();
	});

	test('degraded names the missing capability', async () => {
		const screen = await render(DegradedState, {
			state: { kind: 'degraded', capability: 'TMDB ratings' },
		});
		await expect.element(screen.getByText(/TMDB ratings/)).toBeVisible();
	});

	test('stale shows the age of the evidence rather than a spinner', async () => {
		const screen = await render(StaleState, {
			state: { kind: 'stale', age: '3 days' },
		});
		await expect.element(screen.getByText(/3 days/)).toBeVisible();
		expect(screen.container.querySelector('[data-treatment]')).toBeNull();
	});

	test('pending marks itself in flight rather than settled', async () => {
		const screen = await render(PendingState, {
			state: { kind: 'pending', operation: 'Creating a placeholder' },
		});
		const slot = screen.container.querySelector('[data-slot="pending-state"]');
		expect(slot?.getAttribute('aria-busy')).toBe('true');
	});

	test('blocked shows what is blocked and when it can be retried', async () => {
		const screen = await render(BlockedState, {
			state: {
				kind: 'blocked',
				reason: 'Another browser is holding the setup wizard.',
				retryAfter: '9 minutes',
			},
		});
		await expect
			.element(screen.getByText('Another browser is holding the setup wizard.'))
			.toBeVisible();
		await expect.element(screen.getByText(/9 minutes/)).toBeVisible();
	});

	test('non-convergent names the items that would not settle', async () => {
		const screen = await render(NonConvergentState, {
			state: { kind: 'non-convergent', unsettled: ['Recently Added', 'Hub 4'] },
		});
		await expect.element(screen.getByText('Recently Added')).toBeVisible();
		await expect.element(screen.getByText('Hub 4')).toBeVisible();
	});
});
