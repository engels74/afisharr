<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import {
		LoginForm,
		PlexPinPanel,
		type SignedIn,
		session,
	} from '$lib/features/auth';

	/**
	 * Records the account the sign-in returned, then enters the shell.
	 *
	 * The order is the whole of it. The layout guards every shell route on what
	 * it knows about the session, and what it knows here is the refusal that
	 * sent the operator to this page. Navigating without handing over the
	 * account leaves that refusal standing while the layout's own request is
	 * still in flight — so the shell redirects the operator who has just signed
	 * in straight back to this page (P1).
	 */
	function entered(account: SignedIn) {
		session.adopt(account);
		// `replaceState`, as every other navigation in the shell does. Pushed,
		// Back returns to this page — which `isBareRoute` exempts from the
		// layout's session guard, so a signed-in operator is shown the sign-in
		// form again and a submission from it spends another attempt against
		// the anonymous rate limit.
		void goto('/dashboard', { replaceState: true });
	}
</script>

<div class="flex flex-col gap-8 max-w-md">
	<LoginForm onsignedin={entered} />
	<PlexPinPanel onsignedin={entered} />
</div>
