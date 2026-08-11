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

	/**
	 * How often to ask plex.tv whether the operator has finished.
	 *
	 * Chosen against the budget it spends, not against how fast a code can be
	 * typed. Every poll reaches plex.tv, so every poll costs one of the sixty
	 * provider attempts an address gets each minute — and `trustProxy` is empty
	 * by default, so behind the reverse proxy nearly every deployment runs,
	 * every caller resolves to the proxy's one address and shares that counter.
	 * At two seconds a single panel spent thirty of the sixty, so two operators
	 * signing in at once — or one operator with the page open in two tabs —
	 * refused each other. At five it is twelve, which leaves room for five
	 * concurrent sign-ins and still notices a finished exchange within a few
	 * seconds of the operator finishing it (PRD §21.4.3).
	 */
	const POLL_INTERVAL_MS = 5000;

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
	 * Whether the hosted variant is still worth offering.
	 *
	 * The hosted sign-in needs a return address this instance will stand behind,
	 * and that is `http.publicOrigin` — unset by default, and with no route on
	 * this surface that reports it. Offering the button regardless means a
	 * first-run operator clicks it and is refused every time, with nothing in
	 * the interface ever narrowing what they are being offered.
	 *
	 * So the precondition is learned from the one place that knows it: a
	 * refusal pointing at `/forwardUrl` is the instance saying it cannot honour
	 * a return target. The button goes away for the rest of this visit, the
	 * code variant stays — it needs no return address — and the refusal that
	 * named `http.publicOrigin` stays on screen. Nothing is inferred; the
	 * server was asked and it answered (D-046).
	 */
	let hostedOffered = $state(true);

	/** Whether `problem` is the instance refusing the return target itself. */
	function refusedTheReturnTarget(problem: Problem): boolean {
		return problem.pointer === '/forwardUrl';
	}

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
			if (oauth && refusedTheReturnTarget(result.problem)) {
				hostedOffered = false;
			}
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
				// A spent budget is not a dead attempt. The pin is still open
				// at plex.tv and the operator may already have finished with
				// it, so the timer keeps running and the next poll asks again
				// once the window rolls over — a shared per-address counter
				// makes this the refusal two concurrent sign-ins produce for
				// each other, and throwing the attempt away over it cost both
				// of them a code that was still good (`I-UX-2`).
				if (result.problem.code === 'rateLimited') {
					return;
				}
				clearInterval(timer);
				// And the attempt goes with it. The timer is torn down here and
				// nothing restarts it — `refusal` is not read by this effect —
				// so leaving `started` set renders the "waiting for Plex"
				// branch forever, with the start controls hidden in its
				// `{:else}` and no control anywhere to try again. One transient
				// refusal, a 502 from plex.tv, and the operator's only move is
				// to reload the page (`I-UX-2`).
				started = undefined;
				sessionStorage.removeItem(RESUME_KEY);
				return;
			}
			// The poll that got through clears the refusal the last one showed:
			// a limit that has rolled over is not a fault still in force, and
			// leaving the sentence up would have the operator reading an error
			// beside a sign-in that is working.
			refusal = undefined;
			if (result.value.state === 'authorized') {
				clearInterval(timer);
				// The privilege the session actually carries, and never an
				// assumed one: a linked Plex account that administers nothing
				// gets `is_admin = false`, and recording it as an
				// administrator routes the operator into admin-only pages
				// that then answer 403 (`I-UX-2`).
				onsignedin({
					userId: result.value.userId,
					username: result.value.username,
					isAdmin: result.value.isAdmin,
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
			{#if hostedOffered}
				<button
					class="text-sm underline"
					type="button"
					onclick={() => begin(true)}
				>
					{t('auth.plexOauthStart')}
				</button>
			{/if}
		</div>
	{/if}
</section>
