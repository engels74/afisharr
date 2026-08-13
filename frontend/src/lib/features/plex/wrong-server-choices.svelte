<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { t } from '$lib/shared/i18n';

	/**
	 * The two ways out, heaviest first.
	 *
	 * Ordered by what each one costs rather than alphabetically or by how
	 * likely it is, because the order is the only thing on this surface that
	 * says they are not equivalent: one abandons everything recorded against
	 * the old server, the other puts the old server back. `keeps` carries that
	 * difference as a label rather than as a colour, so it survives both modes
	 * and a reader who is skimming.
	 */
	const ways = $derived([
		{
			key: 'rebind',
			title: t('plex.connection.wrongServer.rebind'),
			body: t('plex.connection.wrongServer.rebindBody'),
			cost: t('plex.connection.wrongServer.rebindCost'),
		},
		{
			key: 'restore',
			title: t('plex.connection.wrongServer.restore'),
			body: t('plex.connection.wrongServer.restoreBody'),
			cost: t('plex.connection.wrongServer.restoreCost'),
		},
	]);
</script>

<!--
	Both ways out, named, and neither taken. `I-ID-5` is a decision the operator
	owns: rebinding abandons every binding recorded against the old server, and
	restoring puts the old server's world back. Choosing either on their behalf
	is the silent rebind the invariant exists to forbid.

	Neither is a control, because neither mechanism exists in this build yet —
	rebinding arrives with the library cache and restore with the backup phase.
	A button that did nothing would be the interface lying about what it can do
	(PRD §8.6), so what is offered is the choice, what each costs, and the one
	move that is available right now.
-->
<div class="flex flex-col gap-3" data-slot="wrong-server-choices">
	<p class="text-sm">{t('plex.connection.wrongServer.unblock')}</p>
	<ul class="flex flex-col gap-3">
		{#each ways as way (way.key)}
			<li class="border-l-2 border-border pl-3" data-way={way.key}>
				<p class="text-sm font-medium">{way.title}</p>
				<p class="text-sm text-muted-foreground">{way.body}</p>
				<p class="text-xs uppercase tracking-wider text-muted-foreground">
					{way.cost}
				</p>
			</li>
		{/each}
	</ul>
	<p class="text-sm">{t('plex.connection.wrongServer.notYet')}</p>
</div>
