<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { api, type Problem } from '$lib/api/client';
	import { ErrorState, LoadingState } from '$lib/components/state';
	import { readSession } from '$lib/features/auth';
	import { landingFor } from '$lib/features/navigation';
	import { t } from '$lib/shared/i18n';

	let startedAt = Date.now();
	let elapsed = $state(0);
	let refusal = $state<Problem | undefined>(undefined);

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
	 *
	 * A health call that fails answers neither question, and it must not be
	 * read as one that did. `data` is undefined for every non-2xx — including
	 * the 503 the client synthesises when the instance cannot be reached at all
	 * — so defaulting it to `false` routes an instance that was configured
	 * months ago into the first-run claim page, which then asks for a console
	 * token that no longer exists and is refused by `require_setup_incomplete`
	 * if it is given one. A five-second restart is enough to produce it. So the
	 * failure is rendered as a failure, with the retry the operator needs
	 * (`I-UX-2`).
	 */
	async function route() {
		// Health is read for the landing decision only. The source link's
		// provenance is recorded by the layout, which every visit passes
		// through — this route runs on a visit to `/` and on nothing else.
		const { data, error } = await api.GET('/api/health');
		if (!data) {
			refusal = error as Problem;
			return;
		}
		const setupCompleted = data.setupCompleted;
		const signedIn =
			setupCompleted && (await readSession()).outcome === 'ok';
		await goto(landingFor(setupCompleted, signedIn), { replaceState: true });
	}

	/** Asks again, from the state the first attempt started in. */
	function retry() {
		refusal = undefined;
		startedAt = Date.now();
		elapsed = 0;
		void route();
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

{#if refusal}
	<ErrorState
		state={{
			kind: 'error',
			summary: refusal.message,
			consequence: t('landing.unreachable'),
		}}
		onretry={retry}
	/>
{:else}
	<LoadingState state={{ kind: 'loading', elapsedMs: elapsed }} rows={2} />
{/if}
