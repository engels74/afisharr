<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { ConnectionPanel } from '$lib/features/plex';
	import { t } from '$lib/shared/i18n';
</script>

<div class="flex flex-col gap-6">
	<h1 class="text-base font-semibold">{t('page.settings.plex')}</h1>
	<!--
		Connectivity and nothing else. Phase 2 binds identity: no library, no
		collection, and no item is behind this page, because a blocked binding
		has to be visible before anything reads a rating key (`I-ID-5`).
	-->
	<ConnectionPanel />
</div>
