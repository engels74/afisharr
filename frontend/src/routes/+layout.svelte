<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	// UnoCSS first, the theme second, and the order is load-bearing. UnoCSS
	// layers are output ordering rather than CSS cascade layers, so its theme
	// block and `app.css`'s `:root` are two unlayered `:root` rules of equal
	// specificity and the last one wins. With `app.css` first, presetWind4's own
	// `--font-sans: var(--font-sans)` landed after ours — a self-referential
	// custom property, invalid at computed-value time, which drops the interface
	// back to the browser's default face while every gate stays green.
	import 'virtual:uno.css';
	import '../app.css';
	import { type Snippet, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api } from '$lib/api/client';
	import { ErrorState, LoadingState } from '$lib/components/state';
	import { ModePreference } from '$lib/features/appearance';
	import { NotPermittedPanel, session } from '$lib/features/auth';
	import {
		isBareRoute,
		LOGIN,
		NavShell,
		SETUP,
		shellFor,
	} from '$lib/features/navigation';
	import { t } from '$lib/shared/i18n';
	import { recordProvenance, SourceFooter } from '$lib/shared/provenance';
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

	/**
	 * What this visit is allowed to see.
	 *
	 * Signed in is not the same as administering this instance: the shell is
	 * admin-only, and so is the stream it opens.
	 */
	const view = $derived(shellFor(bare, session.state));

	/** The instance's own account of why it could not say who is signed in. */
	const unreachable = $derived(
		session.state.kind === 'unreachable' ? session.state.problem : undefined,
	);

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
	 *
	 * The ask goes through `untrack`, and that is not a detail. `refresh()`
	 * touches the session's own in-flight bookkeeping, and anything an effect
	 * reads becomes a dependency of that effect — so an untracked call is the
	 * difference between asking once per navigation and asking again on every
	 * answer, which is a flood of `/api/auth/session` that ends at Svelte's
	 * update-depth guard with nothing rendered at all (P1).
	 */
	$effect(() => {
		// Read as the dependency, so a navigation into the shell asks again and
		// a change inside the session does not.
		const pathname = page.url.pathname;
		if (isBareRoute(pathname)) {
			return;
		}
		untrack(() => {
			void session.refresh();
		});
	});

	$effect(() => {
		// Not while an answer is in flight: `signedOut` recorded on the route
		// before this one is still the state during the request that will
		// replace it, and acting on it here is what sends an operator who has
		// just signed in back to the sign-in page (P1).
		if (bare || session.refreshing) {
			return;
		}
		if (session.state.kind === 'signedOut') {
			void goto(LOGIN, { replaceState: true });
		} else if (session.state.kind === 'setupRequired') {
			// Not the sign-in page: signing in is refused too until setup
			// finishes, so `/login` would be the same dead end one route along.
			void goto(SETUP, { replaceState: true });
		}
	});

	/**
	 * Records what this instance is running, for the footer's source link.
	 *
	 * Asked from here rather than from the root route, because the root route
	 * runs on a visit to `/` and on nothing else: a browser opened on
	 * `/dashboard`, or reloaded anywhere inside the shell, never renders it.
	 * The layout is the one component every visit passes through, and the link
	 * is an AGPL §13 obligation that has to resolve to the running version on
	 * all of them, not only on the visits that happened to start at the root
	 * (D-028).
	 */
	async function loadProvenance() {
		const { data } = await api.GET('/api/health');
		recordProvenance({ version: data?.version });
	}

	$effect(() => {
		void loadProvenance();
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
		// Established after auth, and only where the shell is rendered: the
		// stream carries job progress and source health, its handler requires
		// an administrator, and a client that opened it anyway would spend the
		// whole visit reconnecting into a refusal.
		if (view !== 'shell') {
			return;
		}
		stream.open();
		return () => stream.close();
	});

</script>

<!-- The mode follows the operating system, an explicit choice overrides it,
     and the class is set before paint so a dark instance never flashes light
     on the way in (PRD §10.4, D-050). -->
<ModePreference />

{#snippet notPermitted()}
	<NotPermittedPanel />
{/snippet}

{#snippet sessionUnreachable()}
	{#if unreachable}
		<ErrorState
			state={{
				kind: 'error',
				summary: unreachable.message,
				consequence: t('auth.sessionUnreachable'),
			}}
			onretry={() => session.refresh()}
		/>
	{/if}
{/snippet}

{#snippet waiting()}
	<LoadingState state={{ kind: 'loading', elapsedMs: elapsed }} rows={3} />
{/snippet}

{#snippet framed(body?: Snippet)}
	<!--
		Every view outside the shell renders through here, so all of them carry
		the source link. It is an AGPL §13 obligation owed to the people the
		instance is offered to over a network, and rendering it only inside
		`NavShell` put it behind a signed-in administrator — the one visitor who
		does not need it (D-028, PRD §6.4).
	-->
	<div class="min-h-screen flex flex-col">
		<!-- The same panel the shell gives a page, at the width one form needs:
		     the journeys outside the shell are one column of decisions, and the
		     ground the palette provides is not a reading surface. -->
		<main class="mx-auto w-full max-w-md flex-1 px-4 py-10">
			<!-- The wordmark, because these are the two journeys with no shell
			     around them: without it the sign-in page belongs to no product. -->
			<p class="mb-4 font-serif text-xl font-semibold tracking-tight">
				{t('app.name')}
			</p>
			<div class="rounded-lg border border-border bg-card p-6">
				{@render body?.()}
			</div>
		</main>
		<SourceFooter />
	</div>
{/snippet}

{#if view === 'bare'}
	{@render framed(children)}
{:else if view === 'shell'}
	<NavShell streamStatus={stream.status}>
		{@render children?.()}
	</NavShell>
{:else if page.error}
	<!--
		SvelteKit renders `+error.svelte` as this layout's children, so a view
		that does not render children is a view with no error page. Checked here
		rather than inside each branch: a linked non-admin following a stale
		bookmark was shown the "not an administrator" panel instead of the 404,
		and a page-level error thrown before the session answered was shown as a
		loading skeleton that never resolved.
	-->
	{@render framed(children)}
{:else if view === 'notPermitted'}
	<!--
		Signed in, and not an administrator. Tier 0 is an admin-only surface
		(D-007), so there is no shell to render — and this is not a sign-out
		either: the session is real and the sign-in page would only take them
		back here. What is owed is the sentence saying so (`I-UX-2`).
	-->
	{@render framed(notPermitted)}
{:else if view === 'unreachable' && unreachable}
	<!--
		Asked, and the instance could not say. The retry is the whole point: the
		skeleton below has no navigation on it and renders no children, so a
		restart that answered 502 once left the operator with nothing on screen
		to act on and no page to reach that would have offered one (`I-UX-2`).
	-->
	{@render framed(sessionUnreachable)}
{:else}
	<!--
		Asked and not yet answered, or answered "nobody" and on the way to the
		sign-in page. Neither is a shell, and neither is an error.
	-->
	{@render framed(waiting)}
{/if}
