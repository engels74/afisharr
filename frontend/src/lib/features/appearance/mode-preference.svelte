<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { ModeWatcher, setMode } from 'mode-watcher';
	import { untrack } from 'svelte';
	import { browser } from '$app/environment';
	import { fallbackMode } from './system-mode';

	/**
	 * The mode default, and the one case the library resolves the wrong way.
	 *
	 * `defaultMode` is left at `"system"` and `track` at `true`: following the
	 * operating system is what PRD §10.4 asks for and what the library already
	 * does. What it does not do is the undetectable case — no `matchMedia`, no
	 * query, nothing learned — which it maps to dark. That is the one place a
	 * mode is set here, so a genuine system-dark preference still wins.
	 *
	 * Order against `<ModeWatcher />`'s own mount does not matter, and that is
	 * deliberate: `setMode` writes the choice to local storage synchronously,
	 * so the watcher's mount reads back light whether it ran before this or
	 * after it.
	 *
	 * The write goes through `untrack`, and that is not a detail. `setMode`
	 * reads the library's own persisted-state object on the way to assigning
	 * it, and anything an effect reads becomes a dependency of that effect — so
	 * a tracked call writes what it just read, re-runs, and ends at Svelte's
	 * update-depth guard with nothing rendered at all.
	 */
	$effect(() => {
		const fallback = fallbackMode(browser ? window : undefined);
		if (fallback) {
			untrack(() => setMode(fallback));
		}
	});
</script>

<!--
	The watcher injects a pre-paint script into <head>, which is what keeps a
	dark instance from flashing light on the way in. Applying the class in
	`onMount` instead is the flash this component exists to prevent.
-->
<ModeWatcher />
