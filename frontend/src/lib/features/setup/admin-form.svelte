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
	}

	let { oncreated }: Props = $props();

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
		refusal = result.outcome === 'refused' ? result.problem : undefined;
		if (result.outcome === 'ok') {
			oncreated();
		}
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
