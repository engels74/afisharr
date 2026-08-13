<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { EmptyState } from '$lib/components/state';
	import { t } from '$lib/shared/i18n';
</script>

<!--
	Nothing here carries real data yet: Phase 1 ships the shell, and the page's
	honest state is "nothing created yet" rather than a spinner over a fetch
	that has nowhere to go (PRD §8.3).
-->
<h1 class="text-lg font-semibold">{t('page.homeScreen.title')}</h1>
<EmptyState
	state={{ kind: 'empty', reason: 'nothingCreated' }}
	explanation={t('page.homeScreen.empty')}
/>
