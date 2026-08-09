<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { Problem } from '$lib/api/client';
	import { ErrorState, PendingState } from '$lib/components/state';
	import { t } from '$lib/shared/i18n';
	import {
		type PinStarted,
		pollPlexPin,
		type SignedIn,
		startPlexPin,
	} from './auth-client';

	interface Props {
		onsignedin: (account: SignedIn) => void;
	}

	let { onsignedin }: Props = $props();

	/** How often to ask plex.tv whether the operator has finished. */
	const POLL_INTERVAL_MS = 2000;

	/**
	 * Where an in-flight attempt is kept across the hosted sign-in.
	 *
	 * The OAuth variant leaves this page by top-level navigation and returns to
	 * a fresh document, so an attempt held only in component state is an
	 * attempt nobody polls: plex.tv has authorised a pin this build has
	 * forgotten, and the operator's only move is to start another one. Session
	 * storage rather than local: it belongs to the tab that started it and has
	 * no business outliving it.
	 */
	const RESUME_KEY = 'afisharr.plexAttempt';

	/** The attempt this tab left behind, if it is still worth polling. */
	function resume(): PinStarted | undefined {
		const stored = sessionStorage.getItem(RESUME_KEY);
		// Read once: a pin that was not polled to a conclusion this time is not
		// worth resuming on the load after either.
		sessionStorage.removeItem(RESUME_KEY);
		if (!stored) {
			return undefined;
		}
		try {
			const attempt = JSON.parse(stored) as PinStarted;
			return attempt.expiresAt > Date.now() ? attempt : undefined;
		} catch {
			return undefined;
		}
	}

	let started = $state<PinStarted | undefined>(resume());
	let refusal = $state<Problem | undefined>(undefined);
	let expired = $state(false);

	/**
	 * Starts a sign-in.
	 *
	 * Both variants of the same exchange, and both are reachable: `pin` shows a
	 * four-character code to type at plex.tv/link, and `oauth` sends the
	 * operator to plex.tv's hosted sign-in and brings them back here. The
	 * polling below is identical for the two — the only difference is what the
	 * operator is shown.
	 */
	async function begin(oauth: boolean) {
		refusal = undefined;
		expired = false;
		const result = await startPlexPin(oauth);
		if (result.outcome === 'refused') {
			refusal = result.problem;
			return;
		}
		started = result.value;

		// A top-level navigation, not a popup: the session cookie is
		// `SameSite=Lax`, which withholds it from a cross-site request that is
		// not one. The attempt is parked first, so the document plex.tv returns
		// the operator to picks the same one up rather than orphaning it.
		if (oauth && started.authorizationUrl) {
			sessionStorage.setItem(RESUME_KEY, JSON.stringify(started));
			window.location.assign(started.authorizationUrl);
		}
	}

	$effect(() => {
		const attempt = started;
		if (!attempt || expired) {
			return;
		}
		// One poll in flight at a time. Two overlapping polls both reach
		// plex.tv, and the second is answered by the attempt this one already
		// consumed — which is a refusal the operator would see beside the
		// sign-in that just succeeded.
		let inFlight = false;
		// Polling is a side effect on a timer, which is what `$effect` is for;
		// the state it produces is assigned, never derived from elapsed time.
		const timer = setInterval(async () => {
			if (inFlight) {
				return;
			}
			inFlight = true;
			const result = await pollPlexPin(attempt.id);
			inFlight = false;
			if (result.outcome === 'refused') {
				refusal = result.problem;
				clearInterval(timer);
				return;
			}
			if (result.value.state === 'authorized') {
				clearInterval(timer);
				onsignedin({
					userId: result.value.userId,
					username: result.value.username,
					isAdmin: true,
				});
			}
			if (result.value.state === 'expired') {
				clearInterval(timer);
				expired = true;
			}
		}, POLL_INTERVAL_MS);

		return () => clearInterval(timer);
	});
</script>

<section class="flex flex-col gap-3">
	<h2 class="text-sm font-medium">{t('auth.plexTitle')}</h2>

	{#if refusal}
		<ErrorState state={{ kind: 'error', summary: refusal.message }} />
	{/if}

	{#if expired}
		<p class="text-sm">{t('auth.plexExpired')}</p>
	{/if}

	{#if started && !expired}
		<PendingState state={{ kind: 'pending', operation: t('auth.plexWaiting') }}>
			<p class="text-sm">{t('auth.plexCode', { code: started.code })}</p>
		</PendingState>
	{:else}
		<div class="flex flex-col items-start gap-2">
			<button
				class="text-sm underline"
				type="button"
				onclick={() => begin(false)}
			>
				{t('auth.plexStart')}
			</button>
			<button
				class="text-sm underline"
				type="button"
				onclick={() => begin(true)}
			>
				{t('auth.plexOauthStart')}
			</button>
		</div>
	{/if}
</section>
