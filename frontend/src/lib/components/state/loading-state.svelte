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
			<!--
				`border`, not `muted`, and the reason is arithmetic. The panel a
				page sits on is `card`, and against it `muted` is 1.05:1 in
				light and 1.003:1 in dark — the same color to two decimal
				places. A skeleton nobody can see is an empty panel held for
				the length of the wait, which is the failure the treatment
				exists to prevent. `border` reads at 1.34:1 in both modes.
			-->
			{#each Array.from({ length: rows }, (_, index) => index) as row (row)}
				<div class="h-4 rounded animate-pulse bg-border"></div>
			{/each}
		{:else}
			<p class="text-sm text-muted-foreground">
				{state.progress ?? t('state.loading.stillWorking')}
			</p>
		{/if}
	</div>
{/if}
