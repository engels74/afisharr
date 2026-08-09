<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { ErrorState, LoadingState } from '$lib/components/state';
	import {
		ClaimForm,
		type ClaimStatus,
		readClaimStatus,
		readStatus,
	} from '$lib/features/setup';
	import { t } from '$lib/shared/i18n';

	let status = $state<ClaimStatus | undefined>(undefined);
	let refusal = $state<string | undefined>(undefined);
	let startedAt = Date.now();
	let elapsed = $state(0);

	/**
	 * Every fact this page renders comes from the server.
	 *
	 * `/api/setup/claim` is the one setup read available before the gate is
	 * passed, which is what this page is for. `/api/setup/status` is behind the
	 * gate and is consulted only once the claim is held, to learn whether the
	 * derived step has already moved on.
	 */
	async function load() {
		const gate = await readClaimStatus();
		if (gate.outcome === 'refused') {
			refusal = gate.problem.message;
			return;
		}
		status = gate.value;

		if (!status.claimHeld) {
			return;
		}
		const derived = await readStatus();
		if (derived.outcome === 'ok' && derived.value.step !== 'claim') {
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

{#if refusal}
	<ErrorState state={{ kind: 'error', summary: refusal }} />
{:else if status}
	<p class="text-xs text-[var(--muted-foreground)]">
		{t('setup.step', { ordinal: status.ordinal })}
	</p>
	<ClaimForm {status} onclaimed={() => goto('/setup/admin')} />
{:else}
	<LoadingState state={{ kind: 'loading', elapsedMs: elapsed }} rows={4} />
{/if}
