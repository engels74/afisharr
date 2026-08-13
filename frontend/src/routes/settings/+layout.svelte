<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/state';
	import { isActive, SETTINGS_SUBPAGES } from '$lib/features/navigation';
	import { t } from '$lib/shared/i18n';

	interface Props {
		children?: Snippet;
	}

	let { children }: Props = $props();
</script>

<div class="flex gap-8">
	<nav class="flex flex-col gap-1" aria-label={t('nav.settings')}>
		{#each SETTINGS_SUBPAGES as destination (destination.href)}
			<a
				class="text-sm"
				class:font-medium={isActive(destination, page.url.pathname)}
				aria-current={isActive(destination, page.url.pathname) ? 'page' : undefined}
				href={destination.href}
			>
				{t(destination.label)}
			</a>
		{/each}
	</nav>
	<div class="flex-1">
		{@render children?.()}
	</div>
</div>
