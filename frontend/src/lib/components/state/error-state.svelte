<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { ClassValue } from 'svelte/elements';
	import { t } from '$lib/shared/i18n';
	import { cn } from '$lib/utils';
	import type { SurfaceError } from './surface-state';

	interface Props {
		state: SurfaceError;
		/** Retrying is an action with a visible outcome, never an auto-loop. */
		onretry?: () => void;
		class?: ClassValue;
	}

	let { state, onretry, class: className }: Props = $props();
</script>

<!--
	The destructive token marks the block, not the prose. At `text-sm` it is
	3.76:1 against the card in light and 3.50:1 in dark — enough for a rule,
	which WCAG asks 3:1 of, and short of the 4.5:1 text owes. A red that fails
	on the one state an operator most needs to read is a red chosen for the
	reviewer rather than for the room (P2).
-->
<div
	class={cn(
		'flex flex-col items-start gap-2 border-l-2 border-destructive py-6 pl-3',
		className,
	)}
	data-slot="error-state"
	role="alert"
>
	<p class="text-sm font-medium">{t('state.error.title')}</p>
	<p class="text-sm">{state.summary}</p>
	{#if state.consequence}
		<p class="text-sm text-muted-foreground">{state.consequence}</p>
	{/if}
	{#if onretry}
		<button type="button" class="text-sm underline" onclick={onretry}>
			{t('state.error.retry')}
		</button>
	{/if}
	{#if state.detail}
		<!-- Collapsed, and kept: the operator may be pasting it into a forum. -->
		<details class="text-xs text-muted-foreground">
			<summary>{t('state.error.details')}</summary>
			<!-- The machine's own words, in the machine's face: this is text the
			     operator compares character by character or pastes elsewhere. -->
			<pre class="whitespace-pre-wrap font-mono">{state.detail}</pre>
		</details>
	{/if}
</div>
