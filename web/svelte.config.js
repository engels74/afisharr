// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/**
 * The interface is fully prerendered and embedded in the Rust binary, so there
 * is no JavaScript server runtime in production (PRD §24.4). `fallback` makes
 * every route resolve to one shell that boots client-side; the binary serves
 * that file for any path it does not otherwise answer.
 *
 * @type {import('@sveltejs/kit').Config}
 */
export default {
	preprocess: vitePreprocess(),
	kit: {
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			// `200.html` rather than `index.html`: the root route prerenders to
			// `index.html`, and reusing that name makes the fallback overwrite it.
			fallback: '200.html',
			precompress: false,
			strict: true,
		}),
		alias: { $lib: 'src/lib' },
	},
};
