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
		// Cleared first, so a retry shows the skeleton rather than the refusal
		// it is replacing.
		refusal = undefined;
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
		void load();
	});

	/**
	 * The elapsed counter runs only while the skeleton it feeds is on screen.
	 *
	 * Gated on the state the template branches on, so the effect re-runs when
	 * the answer lands and its cleanup stops the timer. Without the gate the
	 * effect read nothing reactive, ran once, and left a 100 ms tick writing
	 * `$state` for the whole life of the page — and this is the page an
	 * operator leaves open while they go and read the console for the token.
	 */
	$effect(() => {
		if (status || refusal) {
			return;
		}
		const startedAt = Date.now();
		elapsed = 0;
		const ticker = setInterval(() => {
			elapsed = Date.now() - startedAt;
		}, 100);
		return () => clearInterval(ticker);
	});
</script>

{#if refusal}
	<!--
		With a retry, because this is the first page a new instance renders and
		it is outside the shell, so there is no navigation to leave by either.
		The claim read is rate limited and can answer 429 — an operator behind a
		NAT or a reverse proxy shares that budget — and the client turns an
		unreachable instance into a 503, which is what a container restart looks
		like from here. Both pass, and without a control the operator has to know
		to reload the browser themselves (`I-UX-2`).
	-->
	<ErrorState state={{ kind: 'error', summary: refusal }} onretry={load} />
{:else if status}
	<p class="text-xs text-muted-foreground">
		{t('setup.step', { ordinal: status.ordinal })}
	</p>
	<ClaimForm {status} onclaimed={() => goto('/setup/admin')} />
{:else}
	<LoadingState state={{ kind: 'loading', elapsedMs: elapsed }} rows={4} />
{/if}
