<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Problem } from '$lib/api/client';
	import { ErrorState } from '$lib/components/state';
	import { t } from '$lib/shared/i18n';
	import { createAdmin } from './setup-client';

	interface Props {
		oncreated: () => void;
		/**
		 * The claim lapsed while this form was open.
		 *
		 * A separate signal from a refusal, because it is not one this form can
		 * render its way out of: `POST /api/setup/admin` is behind the claim
		 * gate, so once the ten-minute lease lapses every submission answers
		 * the same 403 and this page offers no token field to fix it with. The
		 * page owns where that goes, as it owns every other navigation here.
		 */
		onclaimlost: () => void;
	}

	let { oncreated, onclaimlost }: Props = $props();

	let username = $state('');
	let password = $state('');
	let refusal = $state<Problem | undefined>(undefined);
	let submitting = $state(false);

	const uid = $props.id();
	const usernameId = `${uid}-username`;
	const passwordId = `${uid}-password`;

	/**
	 * The refusal's JSON pointer puts the message beside the field that caused
	 * it, rather than in a banner at the top that names nothing (PRD §8.4).
	 */
	const usernameProblem = $derived(
		refusal?.pointer === '/username' ? refusal.message : undefined,
	);
	const passwordProblem = $derived(
		refusal?.pointer === '/password' ? refusal.message : undefined,
	);
	const generalProblem = $derived(refusal && !refusal.pointer ? refusal : undefined);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		submitting = true;
		const result = await createAdmin(username, password);
		submitting = false;
		if (result.outcome === 'ok') {
			refusal = undefined;
			oncreated();
			return;
		}
		// The bounce back to the claim page lives in the route's `load()`, which
		// runs on mount and never again — so a lease that lapsed while the
		// operator was choosing a password in their manager surfaced here as a
		// banner telling them to enter the token again, on a page with no token
		// field, whose one button reproduces the same 403 for ever.
		//
		// Both refusals, because the claim gate has two: `setupRequired` when
		// the lease is unheld or the renewal lost its race, and `blocked` when
		// another browser now holds it. Reading only the first left the second
		// browser's case — the operator's lease lapses, somebody claims it with
		// the console token — rendering "Another browser is holding the setup
		// wizard." on this page for ever, which is the same dead end wearing the
		// other code.
		if (result.problem.code === 'setupRequired' || result.problem.code === 'blocked') {
			refusal = undefined;
			onclaimlost();
			return;
		}
		refusal = result.problem;
	}
</script>

<section class="flex flex-col gap-4 max-w-md">
	<h1 class="text-lg font-semibold">{t('setup.admin.title')}</h1>
	<p class="text-sm text-[var(--muted-foreground)]">{t('setup.admin.body')}</p>

	{#if generalProblem}
		<ErrorState state={{ kind: 'error', summary: generalProblem.message }} />
	{/if}

	<form class="flex flex-col gap-2" onsubmit={submit}>
		<label class="text-sm" for={usernameId}>{t('auth.username')}</label>
		<input
			id={usernameId}
			class="rounded border border-[var(--border)] px-2 py-1 text-sm"
			bind:value={username}
			autocomplete="username"
		/>
		{#if usernameProblem}
			<p class="text-xs text-[var(--destructive)]">{usernameProblem}</p>
		{/if}

		<label class="text-sm" for={passwordId}>{t('auth.password')}</label>
		<input
			id={passwordId}
			class="rounded border border-[var(--border)] px-2 py-1 text-sm"
			type="password"
			bind:value={password}
			autocomplete="new-password"
		/>
		{#if passwordProblem}
			<p class="text-xs text-[var(--destructive)]">{passwordProblem}</p>
		{/if}

		<button class="self-start text-sm underline" type="submit" disabled={submitting}>
			{t('setup.admin.submit')}
		</button>
	</form>
</section>
