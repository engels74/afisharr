<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { BlockedState, ErrorState, LoadingState } from '$lib/components/state';
	import {
		AdminForm,
		completeSetup,
		readStatus,
		type SetupStatus,
	} from '$lib/features/setup';
	import { t } from '$lib/shared/i18n';

	let status = $state<SetupStatus | undefined>(undefined);
	let blocked = $state<string | undefined>(undefined);
	let refusal = $state<string | undefined>(undefined);
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
		if (status.step === 'claim') {
			// The hold lapsed while this page was open.
			await goto('/setup');
			return;
		}
		if (status.step !== 'admin') {
			// The administrator already exists and setup was interrupted before
			// it finished — a browser closed between the two calls is all it
			// takes. The steps after this one are not part of this build, so
			// bouncing back to the claim page would be a loop with no way out
			// and an instance that can never be signed in to.
			await finish();
		}
	}

	/**
	 * Finishes setup, then sends the operator to sign in.
	 *
	 * Creating the administrator only moves the derived step on. Until setup is
	 * completed `instance.setup_completed_at` is `NULL` and every route behind
	 * `require_setup_completed` is refused — sign-in included — so a redirect
	 * to `/login` without this lands on a page that cannot work.
	 */
	async function finish() {
		const completed = await completeSetup();
		if (completed.outcome === 'refused') {
			refusal = completed.problem.message;
			return;
		}
		await goto('/login');
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
	{#if refusal}
		<ErrorState state={{ kind: 'error', summary: refusal }} />
	{/if}
	<AdminForm oncreated={finish} />
{:else}
	<LoadingState state={{ kind: 'loading', elapsedMs: elapsed }} rows={4} />
{/if}
