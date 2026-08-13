<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ClassValue } from 'svelte/elements';
	import { t } from '$lib/shared/i18n';
	import { cn } from '$lib/utils';
	import { type Consequence, confirmationSatisfied } from './consequence';

	interface Props {
		/** How much this costs if it is wrong, which decides the confirmation. */
		consequence: Consequence;
		/** Specific counts and named objects. Never a summary of the kind. */
		preview: Snippet;
		/** The phrase the operator types, for a typed confirmation. */
		phrase?: string;
		/** What happened, once it has. */
		report?: Snippet;
		onproceed: () => void;
		oncancel?: () => void;
		class?: ClassValue;
	}

	let {
		consequence,
		preview,
		phrase = '',
		report,
		onproceed,
		oncancel,
		class: className,
	}: Props = $props();

	let typed = $state('');
	const satisfied = $derived(confirmationSatisfied(consequence, phrase, typed));
	const confirmId = $props.id();
</script>

<!--
	Preview, confirmation proportional to consequence, report afterwards, and
	never destructive by default (PRD §8.5). The proceed control is not the
	form's default submit and is not focused: it is a plain button that only
	enables once the confirmation is satisfied.
-->
<section class={cn('flex flex-col gap-3', className)} data-slot="destructive-action">
	<div data-slot="destructive-preview">
		<p class="text-sm font-medium">{t('destructive.preview')}</p>
		{@render preview()}
	</div>

	{#if consequence === 'typed'}
		<div class="flex flex-col gap-1">
			<label class="text-sm" for={confirmId}>
				{t('destructive.confirmTyped', { phrase })}
			</label>
			<input
				id={confirmId}
				class="rounded-md border border-border bg-card px-2 py-1 text-sm"
				bind:value={typed}
				autocomplete="off"
			/>
		</div>
	{/if}

	<div class="flex gap-2">
		<button type="button" class="text-sm underline" onclick={oncancel}>
			{t('destructive.cancel')}
		</button>
		<button
			type="button"
			class="rounded bg-destructive px-3 py-1 text-sm text-destructive-foreground disabled:opacity-50"
			disabled={!satisfied}
			onclick={onproceed}
		>
			{t('destructive.proceed')}
		</button>
	</div>

	{#if report}
		<div data-slot="destructive-report">
			<p class="text-sm font-medium">{t('destructive.report')}</p>
			{@render report()}
		</div>
	{/if}
</section>
