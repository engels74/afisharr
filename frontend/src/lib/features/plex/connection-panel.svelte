<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { Problem } from '$lib/api/client';
	import {
		BlockedState,
		EmptyState,
		ErrorState,
		LoadingState,
	} from '$lib/components/state';
	import { t } from '$lib/shared/i18n';
	import ConnectionEvidence from './connection-evidence.svelte';
	import {
		blocks,
		type ConnectionResult,
		checkConnection,
		type PlexConnection,
	} from './plex-client';
	import WrongServerChoices from './wrong-server-choices.svelte';

	let connection = $state<PlexConnection | undefined>(undefined);
	let refusal = $state<Problem | undefined>(undefined);
	let startedAt = $state(Date.now());
	let elapsedMs = $state(0);
	let checking = $state(false);

	/**
	 * Runs the check.
	 *
	 * Guarded against a second click while one is on the wire: the route is
	 * metered against the per-address provider budget, and two checks spend two
	 * of it for one answer.
	 */
	async function check() {
		if (checking) {
			return;
		}
		checking = true;
		refusal = undefined;
		startedAt = Date.now();
		elapsedMs = 0;
		let result: ConnectionResult;
		try {
			result = await checkConnection();
		} finally {
			// In a `finally`, never after the `await`. `checkConnection` promises
			// a value rather than a throw, but the promise can still reject —
			// `openapi-fetch` parses a 2xx body outside the `try` the client's
			// `onError` middleware hooks, so a truncated JSON answer throws past
			// every wrapper. Cleared after the `await` instead, that rejection
			// left this stuck `true`: the button disabled for the life of the
			// page, the elapsed-time interval never torn down, and the panel
			// frozen on "asking the server who it is" with nothing to recover
			// with.
			checking = false;
		}
		if (result.outcome === 'refused') {
			refusal = result.problem;
			return;
		}
		connection = result.value;
	}

	// The check runs once, on load.
	//
	// `untrack`, and it is load-bearing. `check()` reads `checking` for its own
	// re-entry guard, and a read inside an effect is a subscription — so the
	// effect re-ran the moment the guard flipped, hit its own early return, and
	// re-ran again when the request settled. The panel rendered nothing at all:
	// no state, no error, no empty treatment, just the heading and a button. It
	// passed every unit test in this feature, because none of them mounted the
	// panel; a screenshot of the running page is what found it.
	$effect(() => {
		untrack(() => {
			void check();
		});
	});

	// The loading treatment is driven by real elapsed time rather than by a
	// guess: nothing under 300 ms, a skeleton to about three seconds, progress
	// text beyond it (PRD §8.2).
	$effect(() => {
		if (!checking) {
			return;
		}
		const timer = setInterval(() => {
			elapsedMs = Date.now() - startedAt;
		}, 100);
		return () => clearInterval(timer);
	});
</script>

<!--
	Verdict first, evidence under it, and the evidence is the same block in
	every state that has any. A page that rearranged itself per outcome would
	make the operator re-find the identifier each time they checked, and the
	identifier is what they came for.
-->
<section class="flex flex-col gap-5" data-slot="plex-connection">
	<div class="flex items-baseline justify-between gap-4">
		<h2 class="text-sm font-medium">{t('plex.connection.title')}</h2>
		<button
			class="text-sm underline disabled:opacity-50"
			type="button"
			disabled={checking}
			onclick={check}
		>
			{t('plex.connection.check')}
		</button>
	</div>

	<!--
		Beside the verdict rather than instead of it. A refusal is what the last
		attempt did, and it says nothing about what the last completed check
		found: rendered as a branch above `connection`, one 429 against the
		per-address provider budget replaced a blocking `wrongServer` verdict —
		both identifiers, both remedies — with a bare rate-limit sentence, and
		`I-ID-5`'s block vanished off the page while it was still in hand.

		Cleared at the head of every check, so it never sits alongside the
		loading treatment for the attempt that is replacing it.
	-->
	{#if refusal}
		<ErrorState state={{ kind: 'error', summary: refusal.message }} />
	{/if}

	{#if checking}
		<LoadingState
			state={{
				kind: 'loading',
				elapsedMs,
				progress: t('plex.connection.checking'),
			}}
		/>
	{:else if connection}
		<!--
			Re-bound here because the blocked branch renders its choices from a
			snippet, and a snippet is a closure: the narrowing that `{#if
			connection}` performs does not reach inside one.
		-->
		{@const seen = connection}
		<!--
			`blocks` rather than a comparison spelled out here: which states stop
			everything is `I-ID-5`'s rule, and a second copy of it on this page
			is a copy free to disagree with the one the rest of the feature
			narrows on.
		-->
		{#if blocks(seen.state)}
			<BlockedState
				state={{
					kind: 'blocked',
					reason: t('plex.connection.wrongServer.reason'),
				}}
			>
				{#snippet action()}
					<!--
						Evidence before remedies. The claim is that a different
						server answered; the two identifiers are what makes that
						checkable, and an operator asked to choose between
						abandoning their work and restoring a backup should not
						have to scroll past both options to see the proof.
					-->
					<ConnectionEvidence connection={seen} />
					<WrongServerChoices />
				{/snippet}
			</BlockedState>
		{:else if seen.state === 'credentialRefused'}
			<!--
				Its own branch, and above `unreachable` for the reason the state
				exists: the server is there, and the remedy is a sign-in rather
				than a network to go looking through.
			-->
			<ErrorState
				state={{
					kind: 'error',
					summary: t('plex.connection.credentialRefused.title'),
					consequence: t('plex.connection.credentialRefused.consequence'),
					detail: seen.detail ?? undefined,
				}}
			/>
			<ConnectionEvidence connection={seen} />
		{:else if seen.state === 'unreachable'}
			<ErrorState
				state={{
					kind: 'error',
					summary: t('plex.connection.unreachable.title'),
					consequence: t('plex.connection.unreachable.consequence'),
					detail: seen.detail ?? undefined,
				}}
			/>
			<ConnectionEvidence connection={seen} />
		{:else if seen.state === 'reachable'}
			<p class="text-sm">{t('plex.connection.reachable.body')}</p>
			<ConnectionEvidence connection={seen} />
		{:else if seen.state === 'noCredential'}
			<EmptyState
				state={{ kind: 'empty', reason: 'pending' }}
				explanation={t('plex.connection.noCredential.body')}
			/>
			<ConnectionEvidence connection={seen} />
		{:else}
			<EmptyState
				state={{ kind: 'empty', reason: 'nothingCreated' }}
				explanation={t('plex.connection.notConfigured.body')}
			/>
		{/if}
	{/if}
</section>
