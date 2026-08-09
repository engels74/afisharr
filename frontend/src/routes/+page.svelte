<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '$lib/api/client';
	import { LoadingState } from '$lib/components/state';
	import { recordProvenance } from '$lib/shared/provenance';

	let startedAt = Date.now();
	let elapsed = $state(0);

	/**
	 * The root decides where an operator lands, and it asks the instance rather
	 * than guessing: health is the one route that answers without a credential,
	 * and it reports whether setup has finished. An unclaimed instance boots to
	 * the claim page (D-045).
	 */
	async function route() {
		const { data } = await api.GET('/api/health');
		recordProvenance({ version: data?.version });
		await goto(data?.setupCompleted ? '/dashboard' : '/setup', {
			replaceState: true,
		});
	}

	$effect(() => {
		startedAt = Date.now();
		void route();
		const ticker = setInterval(() => {
			elapsed = Date.now() - startedAt;
		}, 100);
		return () => clearInterval(ticker);
	});
</script>

<LoadingState state={{ kind: 'loading', elapsedMs: elapsed }} rows={2} />
