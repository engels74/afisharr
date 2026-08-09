<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { LoadingState } from '$lib/components/state';
	import { ClaimForm, readStatus, type SetupStatus } from '$lib/features/setup';
	import { t } from '$lib/shared/i18n';

	let status = $state<SetupStatus | undefined>(undefined);
	let startedAt = Date.now();
	let elapsed = $state(0);

	async function load() {
		const result = await readStatus();
		// A refusal here is the gate working: an unclaimed instance answers
		// `setupRequired`, which means step one, which is this page.
		status =
			result.outcome === 'ok'
				? result.value
				: {
						step: 'claim',
						ordinal: 1,
						claimHeld: false,
						recoveryAvailable: false,
						tokenLive: true,
					};
		if (status.claimHeld && status.step !== 'claim') {
			await goto('/setup/admin');
		}
	}

	$effect(() => {
		startedAt = Date.now();
		void load();
		const ticker = setInterval(() => {
			elapsed = Date.now() - startedAt;
		}, 100);
		return () => clearInterval(ticker);
	});
</script>

{#if status}
	<p class="text-xs text-[var(--muted-foreground)]">
		{t('setup.step', { ordinal: status.ordinal })}
	</p>
	<ClaimForm {status} onclaimed={() => goto('/setup/admin')} />
{:else}
	<LoadingState state={{ kind: 'loading', elapsedMs: elapsed }} rows={4} />
{/if}
