<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '$lib/api/client';
	import { LoadingState } from '$lib/components/state';
	import { readSession } from '$lib/features/auth';
	import { landingFor } from '$lib/features/navigation';
	import { recordProvenance } from '$lib/shared/provenance';

	let startedAt = Date.now();
	let elapsed = $state(0);

	/**
	 * The root decides where an operator lands, and it asks the instance rather
	 * than guessing. Two questions, because one of them is not enough: health
	 * is the route that answers without a credential and reports whether setup
	 * has finished, and an unclaimed instance boots to the claim page (D-045).
	 * But `setupCompleted` is true for every visitor to a finished instance,
	 * signed in or not — so a claimed instance is asked a second question,
	 * which is the only one that says anything about who is here. Without it,
	 * a signed-out operator lands inside a shell whose every request is refused
	 * and whose stream reconnects and fails, with no sign-in page in sight.
	 */
	async function route() {
		const { data } = await api.GET('/api/health');
		recordProvenance({ version: data?.version });
		const setupCompleted = data?.setupCompleted ?? false;
		const signedIn =
			setupCompleted && (await readSession()).outcome === 'ok';
		await goto(landingFor(setupCompleted, signedIn), { replaceState: true });
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
