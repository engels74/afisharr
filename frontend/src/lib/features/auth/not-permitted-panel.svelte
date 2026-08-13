<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<!--
	Signed in, and not an administrator.

	Tier 0 is an admin-only surface (D-007), so there is no shell to render — and
	this is not a sign-out either: the session is real and the sign-in page would
	only take them back here. What is owed is the sentence saying so, and the one
	action that changes anything (`I-UX-2`).
-->
<script lang="ts">
	import { BlockedState } from '$lib/components/state';
	import { t } from '$lib/shared/i18n';
	import SignOutButton from './sign-out-button.svelte';
</script>

<BlockedState
	state={{
		kind: 'blocked',
		reason: t('auth.notAdministrator'),
	}}
>
	{#snippet action()}
		<SignOutButton />
	{/snippet}
</BlockedState>
