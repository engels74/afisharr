<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ClassValue } from 'svelte/elements';
	import { t } from '$lib/shared/i18n';
	import { cn } from '$lib/utils';
	import type { Blocked } from './surface-state';

	interface Props {
		state: Blocked;
		/** The one action that unblocks it. */
		action?: Snippet;
		class?: ClassValue;
	}

	let { state, action, class: className }: Props = $props();
</script>

<div
	class={cn('flex flex-col items-start gap-2 py-6', className)}
	data-slot="blocked-state"
	role="status"
>
	<p class="text-sm font-medium">{t('state.blocked.title')}</p>
	<p class="text-sm">{state.reason}</p>
	{#if state.retryAfter}
		<p class="text-sm text-[var(--muted-foreground)]">
			{t('state.blocked.retryAt', { duration: state.retryAfter })}
		</p>
	{/if}
	{@render action?.()}
</div>
