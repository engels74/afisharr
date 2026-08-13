<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ClassValue } from 'svelte/elements';
	import { t } from '$lib/shared/i18n';
	import { cn } from '$lib/utils';
	import type { Stale } from './surface-state';

	interface Props {
		state: Stale;
		/** The last known values. Never blank, and never a spinner (PRD §8.6). */
		children?: Snippet;
		class?: ClassValue;
	}

	let { state, children, class: className }: Props = $props();
</script>

<div class={cn('flex flex-col gap-2', className)} data-slot="stale-state">
	<p class="text-xs text-muted-foreground">
		{t('state.stale.title')} · {t('state.stale.body', { age: state.age })}
	</p>
	{@render children?.()}
</div>
