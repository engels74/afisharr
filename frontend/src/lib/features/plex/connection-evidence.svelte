<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { t } from '$lib/shared/i18n';
	import { sharedPrefix } from './identity-diff';
	import type { PlexConnection } from './plex-client';

	interface Props {
		connection: PlexConnection;
	}

	let { connection }: Props = $props();

	/**
	 * Whether the two identifiers are a comparison or a single fact.
	 *
	 * When they agree there is nothing to compare, and rendering the same
	 * string twice under two labels would invite the operator to look for a
	 * difference that is not there.
	 */
	const differs = $derived(
		Boolean(connection.observedMachineIdentifier) &&
			connection.observedMachineIdentifier !== connection.boundMachineIdentifier,
	);

	/** How much of the two identifiers is identical, from the left. */
	const shared = $derived(
		differs
			? sharedPrefix(
					connection.boundMachineIdentifier,
					connection.observedMachineIdentifier,
				)
			: 0,
	);

	/**
	 * The rows, in the order the question is asked: which address, which
	 * server, and — only when it differs — which one answered instead.
	 *
	 * A row the check did not observe is absent rather than blank. "Version:"
	 * with nothing after it is a claim that the server has no version, which is
	 * not what happened (P1).
	 */
	const rows = $derived(
		[
			{
				key: 'address',
				label: t('plex.connection.address'),
				value: connection.baseUrl,
				compared: false,
			},
			{
				key: 'bound',
				label: differs
					? t('plex.connection.bound')
					: t('plex.connection.identity'),
				value: connection.boundMachineIdentifier,
				compared: differs,
			},
			{
				key: 'answered',
				label: t('plex.connection.answered'),
				value: differs ? connection.observedMachineIdentifier : null,
				compared: differs,
			},
		].filter(
			(
				row,
			): row is { key: string; label: string; value: string; compared: boolean } =>
				Boolean(row.value),
		),
	);

	/** The server's own description of itself, when it gave one. */
	const describes = $derived(
		[connection.friendlyName, connection.version].filter(Boolean).join(' · '),
	);
</script>

<!--
	The evidence, and the whole reason this page exists.

	Both identifiers sit in one monospace column, one directly above the other,
	because the operator's question is a character-level comparison between two
	opaque strings. Set inside a sentence, or split across a table of unrelated
	facts, that comparison cannot be made by eye — and it is the only comparison
	`I-ID-5` is about.

	The characters they share are dimmed and the characters they do not are set
	at full strength: a 40-character identifier that differs in one place is
	findable when aligned and obvious when marked. The mark is weight and
	emphasis, never a colour — this page has no palette of its own (§24.3.5.1,
	D-050) — so it survives both modes and a monochrome display.
-->
<dl
	class="grid grid-cols-[max-content_1fr] items-baseline gap-x-6 gap-y-1"
	data-slot="connection-evidence"
>
	{#each rows as row (row.key)}
		<dt
			class="text-xs uppercase tracking-wider text-muted-foreground"
			data-row={row.key}
		>
			{row.label}
		</dt>
		<dd class="font-mono text-sm break-all">
			{#if row.compared}
				<span class="text-muted-foreground">{row.value.slice(0, shared)}</span
				><span class="font-medium">{row.value.slice(shared)}</span>
			{:else}
				{row.value}
			{/if}
		</dd>
	{/each}
</dl>
{#if describes}
	<p class="text-sm text-muted-foreground">{describes}</p>
{/if}
