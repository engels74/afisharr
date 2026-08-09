// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { beforeEach, describe, expect, test } from 'bun:test';
import { recordProvenance, sourceHref } from './source-link';

beforeEach(() => {
	recordProvenance({});
});

describe('the source link', () => {
	test('resolves before the version is known', () => {
		// A footer that renders nothing until a fetch lands is a licence
		// obligation with a loading state.
		expect(sourceHref()).toBe('https://github.com/engels74/afisharr');
	});

	test('points at the running version once it is known', () => {
		recordProvenance({ version: '0.1.0' });
		expect(sourceHref()).toBe(
			'https://github.com/engels74/afisharr/tree/v0.1.0',
		);
	});

	test('a fork can retarget it', () => {
		recordProvenance({
			version: '2.4.0',
			repository: 'https://example.test/fork',
		});
		expect(sourceHref()).toBe('https://example.test/fork/tree/v2.4.0');
	});

	test('a trailing slash on the configured repository does not double up', () => {
		recordProvenance({
			version: '1.0.0',
			repository: 'https://example.test/fork/',
		});
		expect(sourceHref()).toBe('https://example.test/fork/tree/v1.0.0');
	});
});
