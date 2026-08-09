<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { ClassValue } from 'svelte/elements';
	import { t } from '$lib/shared/i18n';
	import { cn } from '$lib/utils';
	import { loadingTreatment } from './loading-policy';
	import type { Loading } from './surface-state';

	interface Props {
		state: Loading;
		/** The shape the skeleton stands in for, so layout does not jump. */
		rows?: number;
		class?: ClassValue;
	}

	let { state, rows = 3, class: className }: Props = $props();

	const treatment = $derived(loadingTreatment(state.elapsedMs));
</script>

{#if treatment !== 'nothing'}
	<div
		class={cn('flex flex-col gap-2', className)}
		data-slot="loading-state"
		data-treatment={treatment}
		role="status"
		aria-live="polite"
		aria-label={t('state.loading.label')}
	>
		{#if treatment === 'skeleton'}
			{#each Array.from({ length: rows }, (_, index) => index) as row (row)}
				<div class="h-4 rounded animate-pulse bg-[var(--muted)]"></div>
			{/each}
		{:else}
			<p class="text-sm text-[var(--muted-foreground)]">
				{state.progress ?? t('state.loading.stillWorking')}
			</p>
		{/if}
	</div>
{/if}
