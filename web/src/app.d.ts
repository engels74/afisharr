// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/// <reference types="bun" />

// The Bun types are referenced here rather than through `compilerOptions.types`
// so the generated SvelteKit tsconfig keeps supplying its own. `bun:test` is
// the unit-test runner for pure `.ts` and `.svelte.ts` modules (§24.3.10).

// See https://svelte.dev/docs/kit/types#app
declare global {
	namespace App {
		// `Locals`, `PageData`, and `Platform` stay empty on purpose: this
		// interface has no JavaScript server runtime (PRD §24.4), so nothing
		// ever populates them.
	}
}

export {};
