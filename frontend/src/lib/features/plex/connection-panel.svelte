<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Problem } from '$lib/api/client';
	import {
		BlockedState,
		EmptyState,
		ErrorState,
		LoadingState,
	} from '$lib/components/state';
	import { t } from '$lib/shared/i18n';
	import ConnectionFacts from './connection-facts.svelte';
	import { checkConnection, type PlexConnection } from './plex-client';
	import WrongServerChoices from './wrong-server-choices.svelte';

	let connection = $state<PlexConnection | undefined>(undefined);
	let refusal = $state<Problem | undefined>(undefined);
	let startedAt = $state(Date.now());
	let elapsedMs = $state(0);
	let checking = $state(false);

	/**
	 * Runs the check.
	 *
	 * Guarded against a second click while one is on the wire: the route is
	 * metered against the per-address provider budget, and two checks spend two
	 * of it for one answer.
	 */
	async function check() {
		if (checking) {
			return;
		}
		checking = true;
		refusal = undefined;
		startedAt = Date.now();
		elapsedMs = 0;
		const result = await checkConnection();
		checking = false;
		if (result.outcome === 'refused') {
			refusal = result.problem;
			return;
		}
		connection = result.value;
	}

	// The check runs on load, and the loading treatment is driven by real
	// elapsed time rather than by a guess: nothing under 300ms, a skeleton to
	// about three seconds, progress text beyond it (PRD §8.2).
	$effect(() => {
		void check();
	});

	$effect(() => {
		if (!checking) {
			return;
		}
		const timer = setInterval(() => {
			elapsedMs = Date.now() - startedAt;
		}, 100);
		return () => clearInterval(timer);
	});
</script>

<section class="flex flex-col gap-4" data-slot="plex-connection">
	<div class="flex items-baseline justify-between gap-4">
		<h2 class="text-sm font-medium">{t('plex.connection.title')}</h2>
		<button
			class="text-sm underline disabled:opacity-50"
			type="button"
			disabled={checking}
			onclick={check}
		>
			{t('plex.connection.check')}
		</button>
	</div>

	{#if checking}
		<LoadingState
			state={{
				kind: 'loading',
				elapsedMs,
				progress: t('plex.connection.checking'),
			}}
		/>
	{:else if refusal}
		<ErrorState state={{ kind: 'error', summary: refusal.message }} />
	{:else if connection}
		<!--
			Re-bound here because the blocked branch renders its choices from a
			snippet, and a snippet is a closure: the narrowing that `{#if
			connection}` performs does not reach inside one.
		-->
		{@const seen = connection}
		{#if seen.state === 'wrongServer'}
			<BlockedState
				state={{
					kind: 'blocked',
					reason: t('plex.connection.wrongServer.reason', {
						address: seen.baseUrl ?? '',
						expected: seen.boundMachineIdentifier ?? '',
						found: seen.observedMachineIdentifier ?? '',
					}),
					unblockLabel: t('plex.connection.wrongServer.unblock'),
				}}
			>
				{#snippet action()}
					<WrongServerChoices connection={seen} />
				{/snippet}
			</BlockedState>
		{:else if seen.state === 'unreachable'}
			<ErrorState
				state={{
					kind: 'error',
					summary: t('plex.connection.unreachable.title'),
					consequence: t('plex.connection.unreachable.consequence'),
					detail: seen.detail ?? undefined,
				}}
			/>
			<ConnectionFacts connection={seen} />
		{:else if seen.state === 'reachable'}
			<p class="text-sm">
				{t('plex.connection.reachable.body', {
					server: seen.friendlyName ?? seen.baseUrl ?? '',
				})}
			</p>
			<ConnectionFacts connection={seen} />
		{:else if seen.state === 'noCredential'}
			<EmptyState
				state={{ kind: 'empty', reason: 'pending' }}
				explanation={t('plex.connection.noCredential.body', {
					address: seen.baseUrl ?? '',
				})}
			/>
		{:else}
			<EmptyState
				state={{ kind: 'empty', reason: 'nothingCreated' }}
				explanation={t('plex.connection.notConfigured.body')}
			/>
		{/if}
	{/if}
</section>
