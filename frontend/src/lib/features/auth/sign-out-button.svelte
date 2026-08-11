<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<!--
	Leaving the account this browser is signed in with.

	One component, because there are two places that need it and the interesting
	part is not the button: it is what happens when the sign-out is refused, and
	two copies of that rule are two chances for one of them to navigate away on
	a failure (P7).

	`LOGIN` is imported from the destinations module directly rather than through
	the navigation feature's index. The index also exports `NavShell`, which
	renders this — so going through it would make the two modules import each
	other at runtime.
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import type { Problem } from '$lib/api/client';
	import { t } from '$lib/shared/i18n';
	import { LOGIN } from '../navigation/destinations';
	import { signOut } from './auth-client';
	import { session } from './session.svelte';

	/** Why the last sign-out did not happen, when it did not. */
	let refusal = $state<Problem | undefined>(undefined);

	/**
	 * Ends this session, and moves only if the instance says it ended.
	 *
	 * `POST /api/auth/logout` can be refused — the anonymous rate limit, the
	 * cross-site check with a token the browser no longer holds — and
	 * navigating anyway left the cookie neither revoked nor expired while the
	 * interface showed the sign-in page. On a shared machine the next person to
	 * open that tab was signed in as the account that thought it had left, so a
	 * refusal stays here and says so.
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

<button class="text-sm underline" type="button" onclick={leave}>
	{t('auth.signOut')}
</button>
{#if refusal}
	<p class="mt-2 text-sm">
		{refusal.message}
		{t('auth.signOutRefused')}
	</p>
{/if}
