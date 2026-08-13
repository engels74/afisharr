<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ClassValue } from 'svelte/elements';
	import { t } from '$lib/shared/i18n';
	import { cn } from '$lib/utils';
	import type { Frozen } from './surface-state';

	interface Props {
		state: Frozen;
		/** The data itself, which is shown and marked — never replaced. */
		children?: Snippet;
		class?: ClassValue;
	}

	let { state, children, class: className }: Props = $props();
</script>

<!--
	Frozen is not an error and must not be styled as one (PRD §8.6). The data is
	shown, marked, with the source and the time of its last success.
-->
<div class={cn('flex flex-col gap-2', className)} data-slot="frozen-state">
	<p class="text-xs text-muted-foreground">
		{t('state.frozen.title')} ·
		{t('state.frozen.body', { source: state.source, age: state.lastSuccessAge })}
	</p>
	{@render children?.()}
</div>
