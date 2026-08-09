// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, test } from 'bun:test';
import { confirmationSatisfied } from './consequence';

describe('confirmation proportional to consequence', () => {
	test('a reversible action needs no phrase', () => {
		expect(confirmationSatisfied('reversible', '', '')).toBe(true);
	});

	test('a typed confirmation needs the exact phrase', () => {
		expect(confirmationSatisfied('typed', 'TEAR DOWN', 'TEAR DOWN')).toBe(true);
		expect(confirmationSatisfied('typed', 'TEAR DOWN', 'tear down')).toBe(
			false,
		);
		expect(confirmationSatisfied('typed', 'TEAR DOWN', 'TEAR')).toBe(false);
	});

	test('surrounding whitespace is forgiven and nothing else is', () => {
		expect(confirmationSatisfied('typed', 'TEAR DOWN', '  TEAR DOWN  ')).toBe(
			true,
		);
		expect(confirmationSatisfied('typed', 'TEAR DOWN', 'TEARDOWN')).toBe(false);
	});

	test('a typed confirmation with no phrase to type is never satisfied', () => {
		// Otherwise a caller that forgot to pass one gets a one-click teardown.
		expect(confirmationSatisfied('typed', '', '')).toBe(false);
	});
});
