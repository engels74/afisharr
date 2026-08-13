<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { t } from '$lib/shared/i18n';
	import type { PlexConnection } from './plex-client';

	interface Props {
		connection: PlexConnection;
	}

	let { connection }: Props = $props();

	/**
	 * The facts the check actually observed, and only those.
	 *
	 * Built as a list rather than as fixed rows so a field the server did not
	 * report is absent rather than shown as a blank value — a row reading
	 * "Version:" with nothing after it is a claim that the server has no
	 * version, which is not what happened (P1).
	 */
	const facts = $derived(
		[
			{ label: t('plex.connection.address'), value: connection.baseUrl },
			{
				label: t('plex.connection.serverName'),
				value: connection.friendlyName,
			},
			{ label: t('plex.connection.version'), value: connection.version },
			{
				label: t('plex.connection.identifier'),
				value: connection.observedMachineIdentifier,
			},
		].filter((fact): fact is { label: string; value: string } =>
			Boolean(fact.value),
		),
	);
</script>

<dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
	{#each facts as fact (fact.label)}
		<dt class="text-muted-foreground">{fact.label}</dt>
		<dd class="font-mono text-xs">{fact.value}</dd>
	{/each}
</dl>
