<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<!--
	The source link, as one component rather than as markup inside the shell.

	AGPL-3.0-or-later section 13 obliges a modified instance offered over a
	network to make its source available to the people it is offered to, and the
	link points at the exact running version (D-028, PRD §6.4). "The people it is
	offered to" is the part that decided the shape here: rendering it only inside
	`NavShell` put it behind a signed-in administrator, so the visitor who loads
	`/login` on a public instance — the network user the section is written for —
	found no link anywhere in the served interface. It is a component so that
	every view the layout can render carries the same one.
-->
<script lang="ts">
	import { t } from '$lib/shared/i18n';
	import { sourceHref } from './source-link.svelte';
</script>

<footer class="border-t border-border">
	<!-- The same column the page is in, so the link sits under the content
	     rather than at the far edge of the window. -->
	<div class="mx-auto w-full max-w-6xl px-4 py-3 text-xs">
		<a class="underline" href={sourceHref()} rel="external">{t('app.sourceLink')}</a>
	</div>
</footer>
