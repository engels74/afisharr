<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/state';
	import { t } from '$lib/shared/i18n';
	import { SourceFooter } from '$lib/shared/provenance';
	import type { StreamStatus } from '$lib/shared/stream';
	import { DisconnectionIndicator } from '$lib/shared/stream';
	import { isActive, PRIMARY, SETTINGS } from './destinations';

	interface Props {
		/** What the live stream is doing, for the non-modal indicator. */
		streamStatus: StreamStatus;
		children?: Snippet;
	}

	let { streamStatus, children }: Props = $props();
</script>

<div class="min-h-screen flex flex-col">
	<a class="sr-only focus:not-sr-only" href="#content">{t('app.skipToContent')}</a>

	<header class="flex items-center gap-4 border-b border-[var(--border)] px-4 py-3">
		<a class="font-semibold" href="/dashboard">{t('app.name')}</a>
		<nav class="flex gap-3" aria-label={t('nav.primary')}>
			{#each PRIMARY as destination (destination.href)}
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
		<div class="ml-auto flex items-center gap-3">
			<DisconnectionIndicator status={streamStatus} />
			<a
				class="text-sm"
				class:font-medium={isActive(SETTINGS, page.url.pathname)}
				href={SETTINGS.href}
			>
				{t(SETTINGS.label)}
			</a>
		</div>
	</header>

	<main class="flex-1 px-4 py-6" id="content">
		{@render children?.()}
	</main>

	<SourceFooter />
</div>
