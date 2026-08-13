<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ClassValue } from 'svelte/elements';
	import { t } from '$lib/shared/i18n';
	import { cn } from '$lib/utils';
	import type { Degraded } from './surface-state';

	interface Props {
		state: Degraded;
		/** The result, which is shown: a missing capability is not a failure. */
		children?: Snippet;
		class?: ClassValue;
	}

	let { state, children, class: className }: Props = $props();
</script>

<div class={cn('flex flex-col gap-2', className)} data-slot="degraded-state">
	<p class="text-xs text-muted-foreground">
		{t('state.degraded.title')} · {state.capability}
		{#if state.configureHref}
			<a class="underline" href={state.configureHref}>{t('state.degraded.configure')}</a>
		{/if}
	</p>
	{@render children?.()}
</div>
