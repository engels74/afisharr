<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/state';
	import { ModeToggle } from '$lib/features/appearance';
	import { t } from '$lib/shared/i18n';
	import { SourceFooter } from '$lib/shared/provenance';
	import type { StreamStatus } from '$lib/shared/stream';
	import { DisconnectionIndicator } from '$lib/shared/stream';
	import { cn } from '$lib/utils';
	import SignOutButton from '../auth/sign-out-button.svelte';
	import { type Destination, isActive, PRIMARY, SETTINGS } from './destinations';

	interface Props {
		/** What the live stream is doing, for the non-modal indicator. */
		streamStatus: StreamStatus;
		children?: Snippet;
	}

	let { streamStatus, children }: Props = $props();
</script>

<!--
	One link treatment, written once and rendered eight times. Composed with
	`cn` rather than with `class:` directives: UnoCSS extracts the classes it
	generates from the source text, and a `class:border-primary` token is not
	one it recognises — the rule was never emitted, so the marker for "you are
	here" rendered as nothing at all while every gate stayed green.
-->
{#snippet link(destination: Destination)}
	{@const here = isActive(destination, page.url.pathname)}
	<a
		class={cn(
			'border-b-2 border-transparent pb-0.5 text-sm text-muted-foreground hover:text-foreground',
			here && 'border-primary font-medium text-foreground',
		)}
		aria-current={here ? 'page' : undefined}
		href={destination.href}
	>
		{t(destination.label)}
	</a>
{/snippet}

<div class="min-h-screen flex flex-col">
	<a class="sr-only focus:not-sr-only" href="#content">{t('app.skipToContent')}</a>

	<!--
		The wordmark is the theme's serif and the only serif in the shell: the
		product's own voice, set once, against an interface that is otherwise
		Inter. Where the operator is stands in the palette's one warm accent, so
		the loudest color in the interface marks a fact rather than a decoration
		(PRD §10.4).
	-->
	<header class="border-b border-border bg-card">
		<div class="mx-auto w-full max-w-6xl flex items-center gap-6 px-4 py-3">
			<a class="font-serif text-xl font-semibold tracking-tight" href="/dashboard">
				{t('app.name')}
			</a>
			<nav class="flex gap-4" aria-label={t('nav.primary')}>
				{#each PRIMARY as destination (destination.href)}
					{@render link(destination)}
				{/each}
			</nav>
			<div class="ml-auto flex items-center gap-4">
				<DisconnectionIndicator status={streamStatus} />
				{@render link(SETTINGS)}
				<!--
					The operator's explicit choice, which overrides the system
					and persists across visits (PRD §10.4). In the shell rather
					than buried in Settings: it is a two-second decision made in
					the room the screen is in, not a configuration item.
				-->
				<ModeToggle />
				<!--
					The administrator's way out. Without it the only caller of
					`signOut()` was the panel shown to non-administrators, so an
					administrator who signed in on a shared or public machine had
					no way to end that session from the interface at all: the
					cookie stayed valid for its full 30-day absolute lifetime,
					and the only remedy was clearing browser cookies.
				-->
				<SignOutButton />
			</div>
		</div>
	</header>

	<!--
		The page sits on `card`, not on `background`. That is what the palette is
		built for — a cool ground with content lifted onto a panel above it — and
		it is also what makes the light mode legible: `muted-foreground` on the
		ground is 4.04:1, under AA, and on the panel it is 4.83:1. A token that
		passes in dark and fails in light is exactly the failure a diff cannot
		show (PRD §10.4).
	-->
	<main class="mx-auto w-full max-w-6xl flex-1 px-4 py-6" id="content">
		<div class="rounded-lg border border-border bg-card p-6">
			{@render children?.()}
		</div>
	</main>

	<SourceFooter />
</div>
