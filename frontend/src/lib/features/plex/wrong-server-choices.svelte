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

	const expected = $derived(connection.boundMachineIdentifier ?? '');
	const found = $derived(connection.observedMachineIdentifier ?? '');
</script>

<!--
	Both ways out, named, and neither taken. `I-ID-5` is a decision the operator
	owns: rebinding abandons every binding recorded against the old server, and
	restoring puts the old server's world back. Choosing either on their behalf
	is the silent rebind the invariant exists to forbid.

	Neither is a control, because neither mechanism exists in this build yet —
	rebinding arrives with the library cache and restore with the backup phase.
	A button that did nothing would be the interface lying about what it can do
	(PRD §8.6), so what is offered is the choice and what each costs, alongside
	the one move that is available now.
-->
<div class="flex flex-col gap-3" data-slot="wrong-server-choices">
	<div class="flex flex-col gap-1">
		<p class="text-sm font-medium">
			{t('plex.connection.wrongServer.rebind')}
		</p>
		<p class="text-sm text-muted-foreground">
			{t('plex.connection.wrongServer.rebindBody', { expected, found })}
		</p>
	</div>
	<div class="flex flex-col gap-1">
		<p class="text-sm font-medium">
			{t('plex.connection.wrongServer.restore')}
		</p>
		<p class="text-sm text-muted-foreground">
			{t('plex.connection.wrongServer.restoreBody')}
		</p>
	</div>
	<p class="text-sm">
		{t('plex.connection.wrongServer.notYet', { expected })}
	</p>
</div>
