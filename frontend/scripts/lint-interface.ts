// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * The two interface rules no general linter can see.
 *
 * Biome checks the language; these check the product. Both are build-failing
 * because both describe a failure that compiles, type-checks, and looks right
 * in review:
 *
 * 1. **No hard-coded user-facing string** (`I-UX-7`). Every user-visible string
 *    resolves through the catalogue, from the first commit. Retrofitting i18n
 *    across every component later is the expensive order.
 * 2. **No state inferred from an HTTP status** (`I-UX-2`). The engine reports
 *    frozen, degraded, stale, pending, blocked, and non-convergent explicitly;
 *    a page that re-derives one from `response.status` or from an array's
 *    length reintroduces in the client exactly the flattening the engine was
 *    built to avoid.
 * 3. **No hand-written request** (§24.5). The generated client is the sole
 *    contract between the two surfaces. A `fetch` with an ad hoc URL is a
 *    second description of the API, and the one that drifts is the one nobody
 *    regenerated.
 */

import { Glob } from 'bun';

/**
 * Where product code lives. Generated output and tests are not product code.
 *
 * `src/lib/shared` is one of them, and leaving it out was an exclusion nothing
 * argued for: it already ships a rendered, screen-reader-announced component
 * (`stream/disconnection-indicator.svelte`), and nothing structurally
 * distinguishes it from `src/lib/components`. A sentence typed into a file
 * there passed every gate — the local run, the `interface-rules` hook, and the
 * CI job — and then rendered in English for every operator whose locale is not,
 * while a hand-written `fetch` beside it sat outside the generated-client
 * contract where `contract-check` could not see it drift (§24.5).
 */
const ROOTS = [
	'src/routes',
	'src/lib/features',
	'src/lib/components',
	'src/lib/shared',
];

/** One rule violation, as the report prints it. */
interface Finding {
	readonly file: string;
	readonly line: number;
	readonly rule: string;
	readonly detail: string;
	readonly source: string;
}

/**
 * Text inside a Svelte template that a person would read.
 *
 * Matches the run of text between `>` and `<` on one line. Deliberately
 * line-based: a template scanner that tried to parse Svelte would be a second
 * Svelte parser, and the failure this catches — somebody typing a sentence
 * into markup — is on one line every time.
 *
 * The run must close on `<`, which is why {@link withoutInterpolation} runs
 * first. `<p>Signed in as {account.username}</p>` produced no match at all
 * before it did: the run `Signed in as ` is followed by `{`, so the gate
 * reported "nothing to report" over the single shape a mixed prose-and-value
 * label always takes, and the English shipped untranslated (`I-UX-7`).
 */
const TEMPLATE_TEXT = />([^<>{}\n]+)</g;

/** A `{...}` expression, innermost first. */
const INTERPOLATION = /\{[^{}]*\}/g;

/**
 * One line with its `{...}` expressions removed.
 *
 * Removing them rather than loosening {@link TEMPLATE_TEXT} to accept `{` and
 * `}` as delimiters, because the scanner reads a `.svelte` file's `<script>`
 * block on the same terms as its markup: a looser pattern read `} catch (error)
 * {` as a sentence somebody had typed into the interface. A line with an
 * unbalanced brace — every block opener and closer in that script — is left
 * exactly as it was, and matches nothing, which is the answer that was already
 * correct for it.
 *
 * Repeated until it settles, so a nested expression collapses whole rather than
 * leaving its outer brace behind.
 */
function withoutInterpolation(line: string): string {
	let text = line;
	for (;;) {
		const stripped = text.replace(INTERPOLATION, '');
		if (stripped === text) {
			return text;
		}
		text = stripped;
	}
}

/** Attributes whose value is read by a person or by a screen reader. */
const USER_FACING_ATTRIBUTES =
	/\s(?:title|placeholder|alt|aria-label|aria-description|aria-placeholder)=["']([^"'{]+)["']/g;

/** Reading a status code to decide what to render. */
const STATUS_BRANCH =
	/(?:response|res|result|error|problem)\s*(?:\?)?\.status\s*(?:===|!==|==|!=|>=|<=|>|<)/;

/** Comparing a status code against a literal in a switch. */
const STATUS_SWITCH = /switch\s*\(\s*[A-Za-z_$][\w$]*\.status\s*\)/;

/** Deciding a display state from how many things came back. */
const LENGTH_BRANCH =
	/\b(?:if|\?|&&|\|\|)\s*\(?\s*[A-Za-z_$][\w$.]*\.length\s*(?:===|!==|==|!=|>|<|>=|<=)\s*0/;

/** A request made without the generated client. */
const HAND_WRITTEN_REQUEST =
	/\b(?:fetch|XMLHttpRequest|axios)\s*\(|new\s+XMLHttpRequest\b/;

/** A line the author has explicitly exempted, with a reason. */
const ALLOW = /afisharr-lint-ignore:\s*(\S+)\s+(.+)/;

/** Text that is not a sentence: symbols, numbers, and single words of markup. */
function isUserFacing(text: string): boolean {
	const trimmed = text.trim();
	if (trimmed.length < 2) {
		return false;
	}
	// Needs at least two letters in a row somewhere, and at least one space or
	// a capital followed by a lower-case run — otherwise it is punctuation, an
	// entity, or a bare token like `px-4`.
	if (!/[A-Za-z]{2}/.test(trimmed)) {
		return false;
	}
	// A lone lower-case identifier-ish token is markup, not a sentence.
	return /\s/.test(trimmed) || /^[A-Z]/.test(trimmed);
}

/** Every source file under the product roots. */
async function sources(): Promise<string[]> {
	const found: string[] = [];
	for (const root of ROOTS) {
		const glob = new Glob('**/*.{svelte,ts}');
		for await (const relative of glob.scan({ cwd: root })) {
			if (relative.endsWith('.test.ts') || relative.includes('generated/')) {
				continue;
			}
			found.push(`${root}/${relative}`);
		}
	}
	return found.sort();
}

/** Checks one file and returns what it found. */
function check(file: string, contents: string): Finding[] {
	const findings: Finding[] = [];
	const lines = contents.split('\n');

	lines.forEach((source, index) => {
		const exemption = ALLOW.exec(source) ?? ALLOW.exec(lines[index - 1] ?? '');
		const exempted = exemption?.[1];

		const record = (rule: string, detail: string) => {
			if (exempted === rule || exempted === 'all') {
				return;
			}
			findings.push({
				file,
				line: index + 1,
				rule,
				detail,
				source: source.trim(),
			});
		};

		if (file.endsWith('.svelte')) {
			const markup = withoutInterpolation(source);
			for (const match of markup.matchAll(TEMPLATE_TEXT)) {
				if (isUserFacing(match[1])) {
					record('no-hardcoded-string', `template text "${match[1].trim()}"`);
				}
			}
			for (const match of source.matchAll(USER_FACING_ATTRIBUTES)) {
				if (isUserFacing(match[1])) {
					record('no-hardcoded-string', `attribute value "${match[1].trim()}"`);
				}
			}
		}

		if (HAND_WRITTEN_REQUEST.test(source)) {
			record(
				'no-hand-written-request',
				'a request made without the generated client',
			);
		}

		if (STATUS_BRANCH.test(source) || STATUS_SWITCH.test(source)) {
			record('no-status-branch', 'a display decision read from an HTTP status');
		}
		if (LENGTH_BRANCH.test(source)) {
			record(
				'no-status-branch',
				'a display decision read from an array length',
			);
		}
	});

	return findings;
}

/** Runs both rules over the product tree and reports. */
async function main(): Promise<number> {
	const files = await sources();
	const findings: Finding[] = [];
	for (const file of files) {
		findings.push(...check(file, await Bun.file(file).text()));
	}

	if (findings.length === 0) {
		console.log(`interface rules: ${files.length} files, nothing to report`);
		return 0;
	}

	for (const finding of findings) {
		console.error(
			`${finding.file}:${finding.line}  ${finding.rule}: ${finding.detail}\n    ${finding.source}`,
		);
	}
	console.error(
		`\n${findings.length} interface-rule violation(s).\n` +
			'Route user-facing text through the catalogue (t/tn), read the state the ' +
			'API returned instead of inferring one, and call the API through the ' +
			'generated client.\n' +
			'A line that genuinely must be exempt carries ' +
			'`afisharr-lint-ignore: <rule> <reason>`.',
	);
	return 1;
}

// Exported for the rule's own tests, which is the only way to be sure a lint
// rule catches what it claims to.
export { check, isUserFacing };

if (import.meta.main) {
	process.exit(await main());
}
