<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import Monitor from '@lucide/svelte/icons/monitor';
	import Moon from '@lucide/svelte/icons/moon';
	import Sun from '@lucide/svelte/icons/sun';
	import { RadioGroup } from 'bits-ui';
	import { setMode, userPrefersMode } from 'mode-watcher';
	import type { ClassValue } from 'svelte/elements';
	import { type MessageKey, t } from '$lib/shared/i18n';
	import { cn } from '$lib/utils';
	import { isModeChoice, MODE_CHOICES, type ModeChoice } from './system-mode';

	interface Props {
		class?: ClassValue;
	}

	let { class: className }: Props = $props();

	/** The stored choice, which is what the control reports — never the mode it resolved to. */
	const chosen = $derived(userPrefersMode.current);

	const LABELS = {
		system: 'appearance.mode.system',
		light: 'appearance.mode.light',
		dark: 'appearance.mode.dark',
	} as const satisfies Record<ModeChoice, MessageKey>;

	const ICONS = { system: Monitor, light: Sun, dark: Moon } as const;

	function choose(value: string): void {
		// A radio group hands back a string, and the one string that is not a
		// choice is the empty deselect. Setting the mode from it would clear a
		// preference the operator never cleared.
		if (isModeChoice(value)) {
			setMode(value);
		}
	}
</script>

<!--
	A radiogroup rather than a light/dark toggle, because the choice is three
	values and one of them is "follow the system". A toggle spends that default
	on its first press and offers no way back to it.

	Bits UI owns the roving focus, the arrow keys, and the ARIA; what is written
	here is which token marks the chosen segment.
-->
<RadioGroup.Root
	class={cn(
		'inline-flex items-center gap-0.5 rounded-md border border-border bg-card p-0.5',
		className,
	)}
	orientation="horizontal"
	value={chosen}
	onValueChange={choose}
	aria-label={t('appearance.mode.label')}
	data-slot="mode-toggle"
>
	{#each MODE_CHOICES as choice (choice)}
		{@const Icon = ICONS[choice]}
		<RadioGroup.Item
			class={cn(
				'rounded-sm p-1.5 text-muted-foreground transition-colors',
				'hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring',
				// `accent`, not `primary`: the warm accent marks where the
				// operator is in the product, and a second thing wearing it in
				// the same header makes neither of them mean anything.
				chosen === choice && 'bg-accent text-accent-foreground',
			)}
			value={choice}
			title={t(LABELS[choice])}
			aria-label={t(LABELS[choice])}
		>
			<Icon class="size-4" aria-hidden="true" />
		</RadioGroup.Item>
	{/each}
</RadioGroup.Root>
