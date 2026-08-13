<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ClassValue } from 'svelte/elements';
	import { t } from '$lib/shared/i18n';
	import { cn } from '$lib/utils';
	import type { NonConvergent } from './surface-state';

	interface Props {
		state: NonConvergent;
		/** The last verified order, which is what is shown. */
		children?: Snippet;
		class?: ClassValue;
	}

	let { state, children, class: className }: Props = $props();
</script>

<div class={cn('flex flex-col gap-2', className)} data-slot="non-convergent-state">
	<p class="text-sm font-medium">{t('state.nonConvergent.title')}</p>
	<p class="text-sm text-muted-foreground">{t('state.nonConvergent.body')}</p>
	<ul class="text-xs text-muted-foreground">
		{#each state.unsettled as item (item)}
			<li>{item}</li>
		{/each}
	</ul>
	{@render children?.()}
</div>
