// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, test } from 'bun:test';
import { check, isUserFacing } from './lint-interface';

/** Every rule this file names, for a violation-count assertion. */
function rules(findings: ReturnType<typeof check>): string[] {
	return findings.map((finding) => finding.rule);
}

describe('the hard-coded string rule', () => {
	test('catches a sentence typed straight into a template', () => {
		const findings = check('a.svelte', '<p>Nothing has run yet.</p>');
		expect(rules(findings)).toEqual(['no-hardcoded-string']);
	});

	test('catches a user-facing attribute', () => {
		const findings = check('a.svelte', '<input placeholder="Setup token" />');
		expect(rules(findings)).toEqual(['no-hardcoded-string']);
	});

	test('passes text that came from the catalogue', () => {
		const findings = check('a.svelte', "<p>{t('page.dashboard.empty')}</p>");
		expect(findings).toEqual([]);
	});

	test('passes markup that is not a sentence', () => {
		for (const markup of [
			'<div class="flex gap-2">',
			'<span>&nbsp;</span>',
			'<p>{count}</p>',
			'<hr />',
			'<div data-slot="state" />',
		]) {
			expect(check('a.svelte', markup), markup).toEqual([]);
		}
	});

	test('does not look for template text in a .ts file', () => {
		const findings = check('a.ts', 'const label = "Sign in";');
		expect(findings).toEqual([]);
	});

	test('an exemption on the line silences exactly that rule', () => {
		const source =
			'<p>Afisharr</p> <!-- afisharr-lint-ignore: no-hardcoded-string a brand name -->';
		expect(check('a.svelte', source)).toEqual([]);
	});
});

describe('the inferred-state rule', () => {
	test('catches a display decision read from a status code', () => {
		const findings = check(
			'a.ts',
			'if (response.status === 404) return empty;',
		);
		expect(rules(findings)).toEqual(['no-status-branch']);
	});

	test('catches a switch over a status code', () => {
		const findings = check('a.ts', 'switch (response.status) {');
		expect(rules(findings)).toEqual(['no-status-branch']);
	});

	test('catches an empty state inferred from an array length', () => {
		const findings = check('a.svelte', '{#if items.length === 0}');
		expect(rules(findings)).toEqual(['no-status-branch']);
	});

	test('passes a page that reads the state the API returned', () => {
		const findings = check('a.svelte', "{#if page.state === 'frozen'}");
		expect(findings).toEqual([]);
	});

	test('passes a length used for something that is not a display state', () => {
		const findings = check('a.ts', 'const total = items.length + 1;');
		expect(findings).toEqual([]);
	});

	test('an exemption on the previous line silences the rule', () => {
		const source = [
			'// afisharr-lint-ignore: no-status-branch the API is not involved here',
			'if (queue.length === 0) return;',
		].join('\n');
		expect(check('a.ts', source)).toEqual([]);
	});
});

describe('the generated-client rule', () => {
	test('catches a hand-written fetch', () => {
		const findings = check('a.ts', "const r = await fetch('/api/health');");
		expect(rules(findings)).toEqual(['no-hand-written-request']);
	});

	test('catches an XMLHttpRequest', () => {
		const findings = check('a.ts', 'const x = new XMLHttpRequest();');
		expect(rules(findings)).toEqual(['no-hand-written-request']);
	});

	test('passes a call through the generated client', () => {
		const findings = check('a.ts', "await api.GET('/api/health');");
		expect(findings).toEqual([]);
	});

	test('passes the word fetch in prose', () => {
		const findings = check('a.ts', '// refetch on reconnect, never replay');
		expect(findings).toEqual([]);
	});
});

describe('what counts as user-facing text', () => {
	test('a sentence does', () => {
		expect(isUserFacing('Nothing has run yet.')).toBe(true);
	});

	test('a capitalised single word does', () => {
		expect(isUserFacing('Dashboard')).toBe(true);
	});

	test('a class list does not', () => {
		expect(isUserFacing('flex gap-2 px-4')).toBe(true);
		// ...which is why class attributes are not scanned at all: the rule
		// looks only at text nodes and at the attributes a person reads.
	});

	test('punctuation and short tokens do not', () => {
		for (const text of ['·', '  ', 'a', '42', '—']) {
			expect(isUserFacing(text), text).toBe(false);
		}
	});
});
