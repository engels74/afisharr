<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ClassValue } from 'svelte/elements';
	import { t } from '$lib/shared/i18n';
	import { cn } from '$lib/utils';
	import type { Empty } from './surface-state';

	interface Props {
		state: Empty;
		/** One line explaining what would be here. */
		explanation: string;
		/** The one thing to do next. */
		action?: Snippet;
		class?: ClassValue;
	}

	let { state, explanation, action, class: className }: Props = $props();
</script>

<!--
	Three kinds, three treatments. "Nothing matched" is never shown for a failed
	fetch: that conflation is the interface expression of P1 (PRD §8.3).
-->
<div
	class={cn('flex flex-col items-start gap-2 py-8', className)}
	data-slot="empty-state"
	data-reason={state.reason}
>
	<p class="text-sm font-medium">
		{#if state.reason === 'nothingCreated'}
			{t('state.empty.nothingCreated.title')}
		{:else if state.reason === 'nothingMatched'}
			{t('state.empty.nothingMatched.title')}
		{:else}
			{t('state.empty.pending.title')}
		{/if}
	</p>
	<p class="text-sm text-muted-foreground">{explanation}</p>
	{#if state.reason === 'nothingMatched' && state.predicate}
		<p class="text-xs text-muted-foreground">
			{t('state.empty.nothingMatched.body', { predicate: state.predicate })}
		</p>
	{/if}
	{@render action?.()}
</div>
