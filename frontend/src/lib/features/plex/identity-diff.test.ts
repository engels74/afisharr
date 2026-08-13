// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, test } from 'bun:test';
import { sharedPrefix } from './identity-diff';

describe('where two identifiers stop agreeing', () => {
	test('a single differing character is found at its own index', () => {
		// The realistic case, and the one the emphasis exists for: forty
		// hexadecimal characters that differ in one place.
		const bound = '9f2c1a77b3e84d6fa0c5e1d29b7a6f38c4e0d512';
		const answered = '9f2c1a77b3e84d6fa0c5e1d29b7a6f38c4e0a512';
		expect(sharedPrefix(bound, answered)).toBe(36);
		expect(bound.slice(36)).toBe('d512');
		expect(answered.slice(36)).toBe('a512');
	});

	test('two identifiers that share nothing share nothing', () => {
		expect(sharedPrefix('abc', 'xyz')).toBe(0);
	});

	test('one identifier that is a prefix of the other stops at its end', () => {
		expect(sharedPrefix('abc', 'abcdef')).toBe(3);
		expect(sharedPrefix('abcdef', 'abc')).toBe(3);
	});

	test('an absent identifier emphasises nothing', () => {
		// The unreachable state has a bound identifier and no observed one.
		// Dimming the whole of the one it does have would say the two agree.
		expect(sharedPrefix('abc', null)).toBe(0);
		expect(sharedPrefix(null, 'abc')).toBe(0);
		expect(sharedPrefix(undefined, undefined)).toBe(0);
		expect(sharedPrefix('', 'abc')).toBe(0);
	});

	test('a split never lands inside a surrogate pair', () => {
		// Plex identifiers are hexadecimal today, and the identifier space is
		// somebody else's to change. A split counted in UTF-16 units would cut
		// an astral character in half and render a replacement box in the one
		// value the operator is comparing.
		const one = '\u{1F600}a';
		const other = '\u{1F600}b';
		const shared = sharedPrefix(one, other);
		expect(shared).toBe(2);
		expect(one.slice(0, shared)).toBe('\u{1F600}');
		expect(one.slice(shared)).toBe('a');
		expect([...one.slice(0, shared)]).toHaveLength(1);
	});
});
