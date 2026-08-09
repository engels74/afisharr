<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import '../app.css';
	import 'virtual:uno.css';
	import { ModeWatcher } from 'mode-watcher';
	import type { Snippet } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { LoadingState } from '$lib/components/state';
	import { createSession } from '$lib/features/auth';
	import { isBareRoute, LOGIN, NavShell } from '$lib/features/navigation';
	import { sourceHref } from '$lib/shared/provenance';
	import { StreamConnection } from '$lib/shared/stream';

	interface Props {
		children?: Snippet;
	}

	let { children }: Props = $props();

	/**
	 * The setup and login journeys are outside the shell: neither has a
	 * navigation bar to offer, because nothing behind it is reachable yet.
	 */
	const bare = $derived(isBareRoute(page.url.pathname));

	const session = createSession();
	const stream = new StreamConnection();

	/**
	 * Every shell route is behind a session, and the shell is where that is
	 * decided.
	 *
	 * `/api/health` says whether the instance has been set up; it says nothing
	 * about who is asking, and it answers the same to a signed-out visitor as
	 * to the operator. So a shell entered on the strength of it is a shell
	 * whose every request answers 401 and whose stream reconnects and fails
	 * forever — the interface insisting it is working while nothing in it does.
	 * The session is asked once per navigation into the shell, and it is the
	 * only thing that decides.
	 */
	$effect(() => {
		if (bare) {
			return;
		}
		void session.refresh();
	});

	$effect(() => {
		if (!bare && session.state.kind === 'signedOut') {
			void goto(LOGIN, { replaceState: true });
		}
	});

	let elapsed = $state(0);
	$effect(() => {
		// `unknown` is not "signed out": on the first load nothing has been
		// asked yet, and rendering either the shell or the sign-in page during
		// that moment would flash the wrong one (P1).
		if (bare || session.state.kind !== 'unknown') {
			return;
		}
		const startedAt = Date.now();
		const ticker = setInterval(() => {
			elapsed = Date.now() - startedAt;
		}, 100);
		return () => clearInterval(ticker);
	});

	$effect(() => {
		// Established after auth, and only where a shell is rendered: the
		// stream carries job progress and source health, it is refused without
		// a session, and a client that opened it anyway would spend the whole
		// visit reconnecting into a 401.
		if (bare || session.state.kind !== 'signedIn') {
			return;
		}
		stream.open();
		return () => stream.close();
	});
</script>

<!-- ModeWatcher sets the theme before paint, so a dark-mode instance never
     flashes light on the way in. -->
<ModeWatcher />

{#if bare}
	<main class="min-h-screen px-4 py-10">
		{@render children?.()}
	</main>
{:else if session.state.kind === 'signedIn'}
	<NavShell streamStatus={stream.status} sourceHref={sourceHref()}>
		{@render children?.()}
	</NavShell>
{:else}
	<!--
		Asked and not yet answered, or answered "nobody" and on the way to the
		sign-in page. Neither is a shell, and neither is an error.
	-->
	<main class="min-h-screen px-4 py-10">
		<LoadingState state={{ kind: 'loading', elapsedMs: elapsed }} rows={3} />
	</main>
{/if}
