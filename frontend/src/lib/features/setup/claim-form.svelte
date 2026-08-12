<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Problem } from '$lib/api/client';
	import { BlockedState, ErrorState } from '$lib/components/state';
	import { formatDuration, t } from '$lib/shared/i18n';
	import { type ClaimStatus, claim, recover } from './setup-client';

	interface Props {
		status: ClaimStatus;
		onclaimed: () => void;
	}

	let { status, onclaimed }: Props = $props();

	let token = $state('');
	let username = $state('');
	let password = $state('');
	let refusal = $state<Problem | undefined>(undefined);
	let submitting = $state(false);

	const uid = $props.id();
	const tokenId = `${uid}-token`;
	const passwordId = `${uid}-password`;
	const usernameId = `${uid}-username`;

	/**
	 * The Blocked treatment carries the retry time the API returned, and is not
	 * derived from the status code: the API says `blocked`, and this reads it
	 * (`I-UX-2`).
	 */
	const blocked = $derived(refusal?.code === 'blocked' ? refusal : undefined);

	async function submitToken(event: SubmitEvent) {
		event.preventDefault();
		submitting = true;
		const result = await claim(token);
		submitting = false;
		refusal = result.outcome === 'refused' ? result.problem : undefined;
		if (result.outcome === 'ok') {
			onclaimed();
		}
	}

	async function submitCredentials(event: SubmitEvent) {
		event.preventDefault();
		submitting = true;
		const result = await recover(username, password);
		submitting = false;
		refusal = result.outcome === 'refused' ? result.problem : undefined;
		if (result.outcome === 'ok') {
			onclaimed();
		}
	}
</script>

<section class="flex flex-col gap-4 max-w-md">
	<h1 class="text-lg font-semibold">{t('setup.claim.title')}</h1>
	<p class="text-sm text-[var(--muted-foreground)]">{t('setup.claim.body')}</p>

	{#if blocked}
		<BlockedState
			state={{
				kind: 'blocked',
				reason: blocked.message,
				retryAfter: blocked.retryAfterSeconds
					? formatDuration(blocked.retryAfterSeconds)
					: undefined,
			}}
		/>
	{:else if refusal}
		<ErrorState state={{ kind: 'error', summary: refusal.message }} />
	{/if}

	{#if status.tokenLive}
		<form class="flex flex-col gap-2" onsubmit={submitToken}>
			<label class="text-sm" for={tokenId}>{t('setup.claim.tokenLabel')}</label>
			<input
				id={tokenId}
				class="rounded border border-[var(--border)] px-2 py-1 text-sm"
				bind:value={token}
				autocomplete="off"
				spellcheck="false"
			/>
			<button class="self-start text-sm underline" type="submit" disabled={submitting}>
				{t('setup.claim.submit')}
			</button>
		</form>
	{:else}
		<p class="text-sm">{t('setup.claim.tokenExpired')}</p>
	{/if}

	{#if status.recoveryAvailable}
		<section class="flex flex-col gap-2 border-t border-[var(--border)] pt-4">
			<h2 class="text-sm font-medium">{t('setup.claim.recoveryTitle')}</h2>
			<p class="text-sm text-[var(--muted-foreground)]">
				{t('setup.claim.recoveryBody')}
			</p>
			<form class="flex flex-col gap-2" onsubmit={submitCredentials}>
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
					{t('setup.claim.recoverySubmit')}
				</button>
			</form>
		</section>
	{/if}
</section>
