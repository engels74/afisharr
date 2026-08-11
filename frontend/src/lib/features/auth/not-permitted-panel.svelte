<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<!--
	Signed in, and not an administrator.

	Tier 0 is an admin-only surface (D-007), so there is no shell to render — and
	this is not a sign-out either: the session is real and the sign-in page would
	only take them back here. What is owed is the sentence saying so, and the one
	action that changes anything (`I-UX-2`).
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import type { Problem } from '$lib/api/client';
	import { BlockedState } from '$lib/components/state';
	import { LOGIN } from '$lib/features/navigation';
	import { t } from '$lib/shared/i18n';
	import { signOut } from './auth-client';
	import { session } from './session.svelte';

	/** Why the last sign-out did not happen, when it did not. */
	let refusal = $state<Problem | undefined>(undefined);

	/**
	 * Leaves an account this interface has nothing to show.
	 *
	 * The answer decides whether anything moves. `POST /api/auth/logout` can be
	 * refused — the anonymous rate limit, the cross-site check with a token the
	 * browser no longer holds — and navigating anyway left the cookie neither
	 * revoked nor expired while the interface showed the sign-in page. On a
	 * shared machine the next person to open that tab was signed in as the
	 * account that thought it had left, so a refusal stays here and says so.
	 */
	async function leave() {
		const result = await signOut();
		if (result.outcome === 'refused') {
			refusal = result.problem;
			return;
		}
		refusal = undefined;
		session.forget();
		await goto(LOGIN, { replaceState: true });
	}
</script>

<BlockedState
	state={{
		kind: 'blocked',
		reason: t('auth.notAdministrator'),
	}}
>
	{#snippet action()}
		<button class="text-sm underline" type="button" onclick={leave}>
			{t('auth.signOut')}
		</button>
		{#if refusal}
			<p class="mt-2 text-sm">
				{refusal.message}
				{t('auth.signOutRefused')}
			</p>
		{/if}
	{/snippet}
</BlockedState>
