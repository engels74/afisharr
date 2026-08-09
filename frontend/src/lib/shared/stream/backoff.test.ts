// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, test } from 'bun:test';
import { BASE_DELAY_MS, backoffDelayMs, MAX_DELAY_MS } from './backoff';

/** A deterministic "random" so the bands can be asserted exactly. */
const lowest = () => 0;
const highest = () => 1;

describe('reconnection backoff', () => {
	test('the first retry waits about a second', () => {
		expect(backoffDelayMs(1, lowest)).toBe(BASE_DELAY_MS / 2);
		expect(backoffDelayMs(1, highest)).toBe(BASE_DELAY_MS);
	});

	test('each retry doubles the band', () => {
		expect(backoffDelayMs(2, highest)).toBe(2000);
		expect(backoffDelayMs(3, highest)).toBe(4000);
		expect(backoffDelayMs(4, highest)).toBe(8000);
	});

	test('the wait is capped', () => {
		for (let attempt = 1; attempt < 40; attempt += 1) {
			expect(backoffDelayMs(attempt, highest)).toBeLessThanOrEqual(
				MAX_DELAY_MS,
			);
		}
		expect(backoffDelayMs(20, highest)).toBe(MAX_DELAY_MS);
	});

	test('the wait is never zero, so a retry loop cannot spin', () => {
		for (let attempt = 1; attempt < 40; attempt += 1) {
			expect(backoffDelayMs(attempt, lowest)).toBeGreaterThan(0);
		}
	});

	test('jitter spreads retries within the band', () => {
		const low = backoffDelayMs(5, lowest);
		const high = backoffDelayMs(5, highest);
		expect(low).toBeLessThan(high);
		expect(high / low).toBeCloseTo(2, 1);
	});
});
