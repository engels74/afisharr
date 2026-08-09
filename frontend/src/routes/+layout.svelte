<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import '../app.css';
	import 'virtual:uno.css';
	import { ModeWatcher } from 'mode-watcher';
	import type { Snippet } from 'svelte';

	interface Props {
		children?: Snippet;
	}

	let { children }: Props = $props();
</script>

<!-- ModeWatcher sets the theme before paint, so a dark-mode instance never
     flashes light on the way in. -->
<ModeWatcher />

{@render children?.()}
