// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, test } from 'bun:test';
import { formatDuration } from './duration';

describe('a wait, in words', () => {
	test('reads a short wait in seconds', () => {
		expect(formatDuration(1)).toBe('1 second');
		expect(formatDuration(30)).toBe('30 seconds');
		expect(formatDuration(59)).toBe('59 seconds');
	});

	test('reads a long wait in minutes', () => {
		// The reason this exists at all: a fifteen-minute lockout rendered as
		// `900s`, which the operator has to divide before it means anything.
		expect(formatDuration(60)).toBe('1 minute');
		expect(formatDuration(900)).toBe('15 minutes');
	});

	test('rounds up, because the number is a floor', () => {
		// "You may try again after this." Rounding down invites a retry that
		// is refused again, and reports the instance as wrong about its own
		// answer.
		expect(formatDuration(61)).toBe('2 minutes');
		expect(formatDuration(0.2)).toBe('1 second');
	});

	test('says something for a wait of nothing at all', () => {
		expect(formatDuration(0)).toBe('1 second');
	});
});
