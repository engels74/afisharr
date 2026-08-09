<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { ErrorState } from '$lib/components/state';
	import { t } from '$lib/shared/i18n';
</script>

<ErrorState
	state={{ kind: 'error', summary: t('page.notFound.title'), consequence: t('page.notFound.body') }}
/>
