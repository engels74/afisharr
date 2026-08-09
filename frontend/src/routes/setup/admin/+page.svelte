<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { BlockedState, LoadingState } from '$lib/components/state';
	import { AdminForm, readStatus, type SetupStatus } from '$lib/features/setup';
	import { t } from '$lib/shared/i18n';

	let status = $state<SetupStatus | undefined>(undefined);
	let blocked = $state<string | undefined>(undefined);
	let startedAt = Date.now();
	let elapsed = $state(0);

	async function load() {
		const result = await readStatus();
		if (result.outcome === 'refused') {
			// The server derives the step; a client that could not read it has
			// not lost its claim, it has been told so (D-046).
			blocked = result.problem.message;
			await goto('/setup');
			return;
		}
		status = result.value;
		if (status.step !== 'admin') {
			await goto('/setup');
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

{#if blocked}
	<BlockedState state={{ kind: 'blocked', reason: blocked }} />
{:else if status}
	<p class="text-xs text-[var(--muted-foreground)]">
		{t('setup.step', { ordinal: status.ordinal })}
	</p>
	<AdminForm oncreated={() => goto('/login')} />
{:else}
	<LoadingState state={{ kind: 'loading', elapsedMs: elapsed }} rows={4} />
{/if}
