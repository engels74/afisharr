<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import {
		BlockedState,
		ErrorState,
		LoadingState,
		PendingState,
	} from '$lib/components/state';
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
	/** Whether `finish()` is in flight, so its wait is not read as its failure. */
	let finishing = $state(false);
	/**
	 * Whether the administrator now exists, so the form must not render again.
	 *
	 * `status` is read once, in `load()`, and completing setup does not change
	 * it — the server's answer to `finish()` is not a status. Without this the
	 * page went on rendering the `admin` step for the whole of `finish()` and
	 * after a refusal of it, so the pending branch and the retry branch below
	 * were both unreachable: the operator watched the form they had just
	 * submitted with no indication anything was happening, and a refused
	 * completion left them on a form whose only button answers 409 "This
	 * instance already has an administrator" for ever.
	 */
	let created = $state(false);
	let elapsed = $state(0);

	/**
	 * Whether a skeleton is what this page is rendering right now.
	 *
	 * The same condition the template branches on, stated once so the timer
	 * that feeds the skeleton cannot outlive it (P7).
	 */
	const loading = $derived(
		!blocked &&
			(!status || ((status.step !== 'admin' || created) && !finishing && !refusal)),
	);

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
	 * Sends the operator back to the claim page, from wherever the claim lapsed.
	 *
	 * `load()` already does this on mount; this is the same journey for a lease
	 * that lapsed later, while the form was open. Without it the operator sits
	 * on a page whose one control answers 403 for ever, and the instance stays
	 * without an administrator.
	 */
	async function returnToClaim() {
		await goto('/setup');
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
		// Set before the call and never cleared: by the time this runs the
		// administrator exists, whatever the completion answers.
		created = true;
		refusal = undefined;
		finishing = true;
		const completed = await completeSetup();
		finishing = false;
		if (completed.outcome === 'refused') {
			refusal = completed.problem.message;
			return;
		}
		await goto('/login');
	}

	$effect(() => {
		void load();
	});

	/**
	 * The elapsed counter runs only while the skeleton it feeds is on screen.
	 *
	 * `loading` is reactive, so the effect re-runs when the answer lands and
	 * its cleanup stops the timer. Ungated, the effect read nothing reactive,
	 * ran once, and left a 100 ms tick writing `$state` for the whole life of
	 * the page — including the time the operator spends filling in the form.
	 */
	$effect(() => {
		if (!loading) {
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

{#if blocked}
	<BlockedState state={{ kind: 'blocked', reason: blocked }} />
{:else if status}
	<p class="text-xs text-muted-foreground">
		{t('setup.step', { ordinal: status.ordinal })}
	</p>
	{#if status.step === 'admin' && !created}
		<AdminForm oncreated={finish} onclaimlost={returnToClaim} />
	{:else if finishing}
		<PendingState state={{ kind: 'pending', operation: t('setup.finish.pending') }} />
	{:else if refusal}
		<!--
			The administrator exists and `finish()` was refused. The form must
			not render here: this instance already has an administrator, so
			every submission answers 409 "Sign in instead", and `finish()` is
			only reachable through the form's `oncreated` — which never fires.
			The operator would be stuck on a page whose one control cannot
			succeed, with a reload the only way out. What the step actually
			needs is the retry for the call that failed (`I-UX-2`).
		-->
		<ErrorState
			state={{
				kind: 'error',
				summary: refusal,
				consequence: t('setup.finish.consequence'),
			}}
			onretry={finish}
		/>
	{:else}
		<LoadingState state={{ kind: 'loading', elapsedMs: elapsed }} rows={2} />
	{/if}
{:else}
	<LoadingState state={{ kind: 'loading', elapsedMs: elapsed }} rows={4} />
{/if}
