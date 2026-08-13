// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { defineConfig, presetWind4, transformerVariantGroup } from 'unocss';

// presetWind4 is the Tailwind-v4-compatible preset and the only one this
// project uses: presetUno and presetWind3 are the superseded names, and the
// shadcn-svelte components are authored against v4 class names and variables.
//
// Every entry below is a reference to a CSS variable `src/app.css` defines, not
// a value. That is the whole point: the tangerine palette is written down once
// (PRD §10.4, D-050), and this file only teaches UnoCSS which utility name
// reaches which token, so `bg-primary` and `--primary` can never drift apart
// (P7). A literal color here would be a second palette.
//
// `presetWebFonts` is deliberately absent. It is the idiomatic way to load the
// three faces the palette names, and it loads them from Google — an outbound
// request carrying the operator's IP on every page load of a product that
// collects nothing (D-038, PRD §21.8). The faces are self-hosted in
// `static/fonts/` and declared with `@font-face` in `src/app.css` instead.
export default defineConfig({
	presets: [presetWind4({ preflights: { reset: true } })],
	transformers: [transformerVariantGroup()],
	theme: {
		// The registry's `cssVars.theme` font families. There is no `@theme`
		// directive on UnoCSS — that is a Tailwind-v4 CSS-file feature — so the
		// families arrive here, and the preflight's `--default-font-family`
		// resolves through `--font-sans` to Inter.
		font: {
			sans: 'var(--font-sans)',
			serif: 'var(--font-serif)',
			mono: 'var(--font-mono)',
		},
		// The registry's `radius: 0.75rem`, as the four-step scale the
		// shadcn-svelte components ask for by name. One knob: `--radius`.
		radius: {
			DEFAULT: 'var(--radius)',
			sm: 'calc(var(--radius) - 4px)',
			md: 'calc(var(--radius) - 2px)',
			lg: 'var(--radius)',
			xl: 'calc(var(--radius) + 4px)',
		},
		colors: {
			background: 'var(--background)',
			foreground: 'var(--foreground)',
			card: {
				DEFAULT: 'var(--card)',
				foreground: 'var(--card-foreground)',
			},
			popover: {
				DEFAULT: 'var(--popover)',
				foreground: 'var(--popover-foreground)',
			},
			primary: {
				DEFAULT: 'var(--primary)',
				foreground: 'var(--primary-foreground)',
			},
			secondary: {
				DEFAULT: 'var(--secondary)',
				foreground: 'var(--secondary-foreground)',
			},
			muted: {
				DEFAULT: 'var(--muted)',
				foreground: 'var(--muted-foreground)',
			},
			accent: {
				DEFAULT: 'var(--accent)',
				foreground: 'var(--accent-foreground)',
			},
			destructive: {
				DEFAULT: 'var(--destructive)',
				foreground: 'var(--destructive-foreground)',
				rule: 'var(--destructive-rule)',
			},
			border: 'var(--border)',
			input: 'var(--input)',
			ring: 'var(--ring)',
			chart: {
				1: 'var(--chart-1)',
				2: 'var(--chart-2)',
				3: 'var(--chart-3)',
				4: 'var(--chart-4)',
				5: 'var(--chart-5)',
			},
			sidebar: {
				DEFAULT: 'var(--sidebar)',
				foreground: 'var(--sidebar-foreground)',
				primary: 'var(--sidebar-primary)',
				'primary-foreground': 'var(--sidebar-primary-foreground)',
				accent: 'var(--sidebar-accent)',
				'accent-foreground': 'var(--sidebar-accent-foreground)',
				border: 'var(--sidebar-border)',
				ring: 'var(--sidebar-ring)',
			},
		},
	},
});
