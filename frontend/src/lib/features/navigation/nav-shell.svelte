<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/state';
	import { t } from '$lib/shared/i18n';
	import type { StreamStatus } from '$lib/shared/stream';
	import { DisconnectionIndicator } from '$lib/shared/stream';
	import { isActive, PRIMARY, SETTINGS } from './destinations';

	interface Props {
		/** What the live stream is doing, for the non-modal indicator. */
		streamStatus: StreamStatus;
		/** Where the source for this exact version lives (AGPL §13, PRD §6.4). */
		sourceHref: string;
		children?: Snippet;
	}

	let { streamStatus, sourceHref, children }: Props = $props();
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

	<!--
		The source link is a licence obligation, not a courtesy: AGPL-3.0-or-later
		section 13 obliges a modified instance offered over a network to make its
		source available, and the link points at the exact running version.
	-->
	<footer class="border-t border-[var(--border)] px-4 py-3 text-xs">
		<a href={sourceHref} rel="external">{t('app.sourceLink')}</a>
	</footer>
</div>
