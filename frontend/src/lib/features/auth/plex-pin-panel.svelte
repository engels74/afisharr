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
	import {
		forgetAttempt,
		POLL_INTERVAL_MS,
		parkAttempt,
		refusedTheReturnTarget,
		resumeAttempt,
	} from './plex-attempt';

	interface Props {
		onsignedin: (account: SignedIn) => void;
	}

	let { onsignedin }: Props = $props();

	let started = $state<PinStarted | undefined>(resumeAttempt());
	let refusal = $state<Problem | undefined>(undefined);
	let expired = $state(false);
	/**
	 * Whether a start is on the wire, so a second click cannot send another.
	 *
	 * Every other form on this surface guards its submit. Without it a double
	 * click sent two `POST /api/auth/plex/pin`, spent two of the sixty provider
	 * attempts an address gets a minute — which the interval above is already
	 * chosen tightly against — and orphaned the first pin, because the second
	 * answer overwrites `started` and nothing ever polls the first to a
	 * conclusion. On the hosted variant it also called
	 * `window.location.assign` twice.
	 */
	let starting = $state(false);

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
		if (starting) {
			return;
		}
		starting = true;
		refusal = undefined;
		// The old attempt goes before the new one is asked for, and that order
		// is the whole of it. `expired` was cleared here while `started` still
		// held the dead pin, so the waiting branch matched again and the panel
		// re-rendered the expired four-character code under "Waiting for
		// Plex…", with the polling effect restarted on it. A start that is then
		// refused — `rateLimited` is the ordinary one here, since two
		// concurrent sign-ins share the per-address provider budget — left that
		// standing: the start buttons live in the `{:else}`, so there was no
		// control left to try again and the operator sat typing a dead code at
		// plex.tv (`I-UX-2`).
		started = undefined;
		expired = false;
		const result = await startPlexPin(oauth);
		starting = false;
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
			parkAttempt(started);
			window.location.assign(started.authorizationUrl);
		}
	}

	$effect(() => {
		const attempt = started;
		if (!attempt || expired) {
			return;
		}
		// One poll in flight at a time. Two overlapping polls both reach plex.tv,
		// and the second is answered by the attempt this one already consumed —
		// a refusal the operator would see beside the sign-in that succeeded.
		let inFlight = false;
		// Whether this run is still the live one. `inFlight` is scoped to one
		// run, so a poll still on the wire at teardown resolves afterwards and
		// writes into whatever attempt is live by then: a stale `forgetAttempt()`
		// drops the entry `begin()` has just parked for the hosted sign-in, a
		// stale `expired` renders a brand-new pin as expired, and a stale
		// `authorized` signs the operator in after they navigated away.
		let cancelled = false;
		// Polling is a side effect on a timer, which is what `$effect` is for;
		// the state it produces is assigned, never derived from elapsed time.
		const timer = setInterval(async () => {
			if (inFlight) {
				return;
			}
			inFlight = true;
			let result: Awaited<ReturnType<typeof pollPlexPin>>;
			try {
				result = await pollPlexPin(attempt.id);
			} finally {
				// In a `finally`, never after the `await`: a rejection there left
				// this stuck `true`, so every later tick returned at the guard
				// above and the panel showed "Waiting for Plex…" for the life of
				// the page, with nothing to recover with.
				inFlight = false;
			}
			if (cancelled) {
				return;
			}
			if (result.outcome === 'refused') {
				refusal = result.problem;
				// A spent budget is not a dead attempt. The pin is still open
				// at plex.tv and the operator may already have finished with
				// it, so the timer keeps running and the next poll asks again
				// once the window rolls over — a shared per-address counter
				// makes this the refusal two concurrent sign-ins produce for
				// each other, and throwing the attempt away over it cost both
				// of them a code that was still good (`I-UX-2`).
				// The same holds for an upstream refusal, and for the same
				// reason. `upstream` is plex.tv answering something this
				// instance could not use — a 429 or a 5xx on the account
				// lookup, which happens *after* plex.tv has already reported
				// the pin authorised, so the operator has finished their part
				// and the pin is still open. Treated as fatal, one transient
				// plex.tv hiccup threw away an attempt that would have
				// completed on the very next poll (`I-UX-2`).
				if (
					result.problem.code === 'rateLimited' ||
					result.problem.code === 'upstream'
				) {
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
				forgetAttempt();
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

		return () => {
			cancelled = true;
			clearInterval(timer);
		};
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
				disabled={starting}
				onclick={() => begin(false)}
			>
				{t('auth.plexStart')}
			</button>
			{#if hostedOffered}
				<button
					class="text-sm underline"
					type="button"
					disabled={starting}
					onclick={() => begin(true)}
				>
					{t('auth.plexOauthStart')}
				</button>
			{/if}
		</div>
	{/if}
</section>
