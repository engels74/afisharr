<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Problem } from '$lib/api/client';
	import { BlockedState, ErrorState } from '$lib/components/state';
	import { t } from '$lib/shared/i18n';
	import { type SignedIn, signIn } from './auth-client';

	interface Props {
		onsignedin: (account: SignedIn) => void;
	}

	let { onsignedin }: Props = $props();

	let username = $state('');
	let password = $state('');
	let refusal = $state<Problem | undefined>(undefined);
	let submitting = $state(false);

	const uid = $props.id();
	const usernameId = `${uid}-username`;
	const passwordId = `${uid}-password`;

	// Read from the code the API returned, not from the status number.
	const limited = $derived(refusal?.code === 'rateLimited' ? refusal : undefined);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		submitting = true;
		const result = await signIn(username, password);
		submitting = false;
		refusal = result.outcome === 'refused' ? result.problem : undefined;
		if (result.outcome === 'ok') {
			onsignedin(result.value);
		}
	}
</script>

<section class="flex flex-col gap-4 max-w-md">
	<h1 class="text-lg font-semibold">{t('auth.title')}</h1>

	{#if limited}
		<BlockedState
			state={{
				kind: 'blocked',
				reason: limited.message,
				retryAfter: limited.retryAfterSeconds
					? `${limited.retryAfterSeconds}s`
					: undefined,
			}}
		/>
	{:else if refusal}
		<ErrorState state={{ kind: 'error', summary: refusal.message }} />
	{/if}

	<form class="flex flex-col gap-2" onsubmit={submit}>
		<label class="text-sm" for={usernameId}>{t('auth.username')}</label>
		<input
			id={usernameId}
			class="rounded border border-[var(--border)] px-2 py-1 text-sm"
			bind:value={username}
			autocomplete="username"
		/>
		<label class="text-sm" for={passwordId}>{t('auth.password')}</label>
		<input
			id={passwordId}
			class="rounded border border-[var(--border)] px-2 py-1 text-sm"
			type="password"
			bind:value={password}
			autocomplete="current-password"
		/>
		<button class="self-start text-sm underline" type="submit" disabled={submitting}>
			{t('auth.signIn')}
		</button>
	</form>
</section>
