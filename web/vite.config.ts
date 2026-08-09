// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { sveltekit } from '@sveltejs/kit/vite';
import UnoCSS from 'unocss/vite';
import { defineConfig } from 'vite';

// UnoCSS before sveltekit: the generated stylesheet has to exist before the
// Svelte plugin resolves `virtual:uno.css`.
export default defineConfig({
	plugins: [UnoCSS(), sveltekit()],
});
