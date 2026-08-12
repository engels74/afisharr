// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { sveltekit } from '@sveltejs/kit/vite';
import { playwright } from '@vitest/browser-playwright';
import UnoCSS from 'unocss/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	// The same pair as the application build, in the same order: UnoCSS first,
	// because the Svelte plugin resolves `virtual:uno.css`, and the layout —
	// the one component every visit passes through — imports it.
	plugins: [UnoCSS(), sveltekit()],
	test: {
		include: ['src/**/*.svelte.test.ts'],
		// Left at its default (`false`), deliberately. `bunfig.toml` sets
		// `pathIgnorePatterns` for `*.svelte.test.ts`, so this lane is the only
		// place a component is tested at all — and with the flag on, anything
		// that made the glob above match nothing (a rename, a directory move, a
		// plugin that fails to resolve the specs) reported success on zero
		// specs. Both frontend jobs went green, and a sign-in page that
		// rendered blank would have reached main with nothing saying the suite
		// was empty.
		setupFiles: ['vitest-browser-svelte'],
		browser: {
			enabled: true,
			provider: playwright(),
			headless: true,
			instances: [{ browser: 'chromium' }],
		},
	},
});
