<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { EmptyState } from '$lib/components/state';
	import { t } from '$lib/shared/i18n';
</script>

<h1 class="text-base font-semibold">{t('page.settings.about')}</h1>
<EmptyState
	state={{ kind: 'empty', reason: 'nothingCreated' }}
	explanation={t('page.settings.empty')}
/>
