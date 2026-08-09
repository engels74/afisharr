<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { LoginForm, PlexPinPanel } from '$lib/features/auth';
</script>

<div class="flex flex-col gap-8 max-w-md">
	<LoginForm onsignedin={() => goto('/dashboard')} />
	<PlexPinPanel onsignedin={() => goto('/dashboard')} />
</div>
