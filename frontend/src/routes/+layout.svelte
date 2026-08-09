<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import '../app.css';
	import 'virtual:uno.css';
	import { ModeWatcher } from 'mode-watcher';
	import type { Snippet } from 'svelte';
	import { page } from '$app/state';
	import { NavShell } from '$lib/features/navigation';
	import { sourceHref } from '$lib/shared/provenance';
	import { StreamConnection } from '$lib/shared/stream';

	interface Props {
		children?: Snippet;
	}

	let { children }: Props = $props();

	/**
	 * The setup and login journeys are outside the shell: neither has a
	 * navigation bar to offer, because nothing behind it is reachable yet.
	 */
	const bare = $derived(
		page.url.pathname.startsWith('/setup') || page.url.pathname === '/login',
	);

	const stream = new StreamConnection();

	$effect(() => {
		if (bare) {
			return;
		}
		// Established after auth, and only where a shell is rendered: the
		// stream carries job progress and source health, and an unclaimed
		// instance has neither.
		stream.open();
		return () => stream.close();
	});
</script>

<!-- ModeWatcher sets the theme before paint, so a dark-mode instance never
     flashes light on the way in. -->
<ModeWatcher />

{#if bare}
	<main class="min-h-screen px-4 py-10">
		{@render children?.()}
	</main>
{:else}
	<NavShell streamStatus={stream.status} sourceHref={sourceHref()}>
		{@render children?.()}
	</NavShell>
{/if}
