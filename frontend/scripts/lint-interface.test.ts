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

	test('catches prose sitting beside an interpolated value', () => {
		// The shape every mixed prose-and-value label takes, and the one the
		// rule was blind to: the run of text closes on `{` rather than on `<`,
		// so the scanner found nothing and reported success while the English
		// shipped untranslated.
		for (const markup of [
			'<p>Signed in as {account.username}</p>',
			'<h2>Waiting for {source.label} to answer</h2>',
			'<button>Retry in {seconds}s</button>',
		]) {
			expect(rules(check('a.svelte', markup)), markup).toEqual([
				'no-hardcoded-string',
			]);
		}
	});

	test('catches a sentence the formatter broke across lines', () => {
		// The shape the gate was blind to, and the shape it is most likely to
		// meet: the rule exists for sentences, sentences are long, and Biome
		// wraps a long element. Reported on the line the sentence is on, not on
		// the line the opening tag ends on.
		const source = [
			'<p class="text-sm text-[var(--muted-foreground)]">',
			'\tYour account does not administer this instance.',
			'</p>',
		].join('\n');
		const findings = check('a.svelte', source);
		expect(rules(findings)).toEqual(['no-hardcoded-string']);
		expect(findings[0]?.line).toBe(2);
	});

	test('catches wrapped prose sitting beside an interpolated value', () => {
		const source = [
			'<h2>',
			'\tWaiting for {source.label} to answer',
			'</h2>',
		].join('\n');
		expect(rules(check('a.svelte', source))).toEqual(['no-hardcoded-string']);
	});

	test('reports a wrapped sentence once, not twice', () => {
		// The per-line rule and the wrapped rule scan the same file, so a run
		// that both could see would be two findings for one sentence.
		const source = ['<p>', '\tNothing has run yet.', '</p>'].join('\n');
		expect(check('a.svelte', source)).toHaveLength(1);
	});

	test('an exemption above a wrapped sentence silences it', () => {
		const source = [
			'<!-- afisharr-lint-ignore: no-hardcoded-string a brand name -->',
			'<p>',
			'\tAfisharr keeps its own name.',
			'</p>',
		].join('\n');
		expect(check('a.svelte', source)).toEqual([]);
	});

	test('passes a multi-line comparison in a script block', () => {
		// Why the wrapped rule runs over the markup alone: between the `>` of
		// one comparison and the `<` of the next lies a run with letters and
		// spaces in it, which is everything `isUserFacing` asks for.
		const source = [
			'<script lang="ts">',
			'\tconst wide = width > 3;',
			'\tconst narrow = height < 4;',
			'</script>',
		].join('\n');
		expect(check('a.svelte', source)).toEqual([]);
	});

	test('passes a script line whose braces open and close a block', () => {
		// The reason the interpolation is removed rather than the pattern
		// loosened: this scanner reads a `.svelte` file's script block on the
		// same terms as its markup, and a looser pattern read these as
		// sentences somebody had typed into the interface.
		for (const line of [
			'\t} catch (error) {',
			'\t} else if (session.refreshing) {',
			'\tlet { children }: Props = $props();',
		]) {
			expect(check('a.svelte', line), line).toEqual([]);
		}
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
