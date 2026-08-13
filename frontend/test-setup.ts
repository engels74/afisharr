// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

// `bun test` covers pure `.ts` and `.svelte.ts` modules; some of them touch DOM
// APIs, so a DOM is registered before any test file loads. Component rendering
// belongs to Vitest browser mode, not here.
import { GlobalRegistrator } from '@happy-dom/global-registrator';
import { compileModule } from 'svelte/compiler';

// With a document location, because `about:blank` cannot resolve a relative
// URL: every request this interface makes is same-origin and relative, so a DOM
// registered without one turns the first `api.GET('/api/...')` into a
// `DOMException` about the document rather than an answer about the API.
GlobalRegistrator.register({ url: 'http://localhost/' });

// Runes are compiler primitives, not functions Bun can resolve, so a
// `.svelte.ts` module loaded raw fails at `$state is not defined`. Vite runs
// the same compiler for the application build; this is the test runner's half
// of it, and it is what lets a rune module be unit-tested where the rule file
// says it should be rather than being pushed into the browser lane.
const stripTypes = new Bun.Transpiler({ loader: 'ts' });

Bun.plugin({
	name: 'svelte-rune-modules',
	setup(build) {
		build.onLoad({ filter: /\.svelte\.ts$/ }, async ({ path }) => {
			const source = await Bun.file(path).text();
			// Two passes, because each tool reads one language: Bun's
			// transpiler takes the TypeScript out, and Svelte's module
			// compiler turns what is left into rune-aware JavaScript.
			const javascript = stripTypes.transformSync(source);
			const compiled = compileModule(javascript, {
				filename: path,
				generate: 'client',
			});
			return { contents: compiled.js.code, loader: 'js' };
		});
	},
});
