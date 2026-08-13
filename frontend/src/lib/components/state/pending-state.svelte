<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ClassValue } from 'svelte/elements';
	import { t } from '$lib/shared/i18n';
	import { cn } from '$lib/utils';
	import type { Pending } from './surface-state';

	interface Props {
		state: Pending;
		/** The optimistic value, shown as in-flight and distinct from settled. */
		children?: Snippet;
		class?: ClassValue;
	}

	let { state, children, class: className }: Props = $props();
</script>

<!--
	Never rendered as done. Placement in particular: a move is pending until
	read-back confirms it (PRD §8.6, I-UX-4).
-->
<div
	class={cn('flex flex-col gap-2 opacity-70', className)}
	data-slot="pending-state"
	aria-busy="true"
>
	<p class="text-xs text-muted-foreground">
		{t('state.pending.title')} · {state.operation}
	</p>
	{@render children?.()}
</div>
