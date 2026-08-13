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

<div
	class={cn('flex flex-col items-start gap-2 py-6', className)}
	data-slot="error-state"
	role="alert"
>
	<p class="text-sm font-medium">{t('state.error.title')}</p>
	<p class="text-sm">{state.summary}</p>
	{#if state.consequence}
		<p class="text-sm text-[var(--muted-foreground)]">{state.consequence}</p>
	{/if}
	{#if onretry}
		<button type="button" class="text-sm underline" onclick={onretry}>
			{t('state.error.retry')}
		</button>
	{/if}
	{#if state.detail}
		<!-- Collapsed, and kept: the operator may be pasting it into a forum. -->
		<details class="text-xs text-[var(--muted-foreground)]">
			<summary>{t('state.error.details')}</summary>
			<pre class="whitespace-pre-wrap">{state.detail}</pre>
		</details>
	{/if}
</div>
