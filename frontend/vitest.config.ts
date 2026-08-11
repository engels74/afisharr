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
		// No `.svelte` component exists yet — the interface shell is Phase 1.
		// The lane is wired now so the first component to land is covered by a
		// lane that already runs, rather than by one somebody has to build
		// alongside it.
		passWithNoTests: true,
		setupFiles: ['vitest-browser-svelte'],
		browser: {
			enabled: true,
			provider: playwright(),
			headless: true,
			instances: [{ browser: 'chromium' }],
		},
	},
});
