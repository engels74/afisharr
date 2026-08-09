<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Problem } from '$lib/api/client';
	import { ErrorState, PendingState } from '$lib/components/state';
	import { t } from '$lib/shared/i18n';
	import {
		type PinStarted,
		pollPlexPin,
		type SignedIn,
		startPlexPin,
	} from './auth-client';

	interface Props {
		onsignedin: (account: SignedIn) => void;
	}

	let { onsignedin }: Props = $props();

	let started = $state<PinStarted | undefined>(undefined);
	let refusal = $state<Problem | undefined>(undefined);
	let expired = $state(false);

	/** How often to ask plex.tv whether the operator has finished. */
	const POLL_INTERVAL_MS = 2000;

	async function begin() {
		refusal = undefined;
		expired = false;
		const result = await startPlexPin(false);
		if (result.outcome === 'refused') {
			refusal = result.problem;
			return;
		}
		started = result.value;
	}

	$effect(() => {
		const attempt = started;
		if (!attempt || expired) {
			return;
		}
		// Polling is a side effect on a timer, which is what `$effect` is for;
		// the state it produces is assigned, never derived from elapsed time.
		const timer = setInterval(async () => {
			const result = await pollPlexPin(attempt.id);
			if (result.outcome === 'refused') {
				refusal = result.problem;
				clearInterval(timer);
				return;
			}
			if (result.value.state === 'authorized') {
				clearInterval(timer);
				onsignedin({
					userId: result.value.userId,
					username: result.value.username,
					isAdmin: true,
				});
			}
			if (result.value.state === 'expired') {
				clearInterval(timer);
				expired = true;
			}
		}, POLL_INTERVAL_MS);

		return () => clearInterval(timer);
	});
</script>

<section class="flex flex-col gap-3">
	<h2 class="text-sm font-medium">{t('auth.plexTitle')}</h2>

	{#if refusal}
		<ErrorState state={{ kind: 'error', summary: refusal.message }} />
	{/if}

	{#if expired}
		<p class="text-sm">{t('auth.plexExpired')}</p>
	{/if}

	{#if started && !expired}
		<PendingState state={{ kind: 'pending', operation: t('auth.plexWaiting') }}>
			<p class="text-sm">{t('auth.plexCode', { code: started.code })}</p>
		</PendingState>
	{:else}
		<button class="self-start text-sm underline" type="button" onclick={begin}>
			{t('auth.plexStart')}
		</button>
	{/if}
</section>
