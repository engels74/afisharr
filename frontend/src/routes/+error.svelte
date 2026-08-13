<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import { page } from '$app/state';
	import { ErrorState } from '$lib/components/state';
	import { t } from '$lib/shared/i18n';

	/**
	 * A missing page and a failed one are different facts, and this component
	 * renders both.
	 *
	 * SvelteKit hands every page-level failure to this file — an unmatched
	 * client route, an `error(...)` thrown while rendering, and an uncaught
	 * exception in a component alike. Answered with the not-found sentence for
	 * all of them, a render that failed told the operator that the address does
	 * not resolve, and sent them looking for a typo in a URL that is correct
	 * (`I-UX-2`).
	 *
	 * `page.status` is the status SvelteKit *reported*, not one inferred from a
	 * response body, which is why reading it here is not the flattening that
	 * rule forbids. The message is the catalogue's rather than
	 * `page.error.message`: SvelteKit's own default text is English typed into
	 * the framework, and rendering it would put an untranslated sentence in
	 * front of every operator whose locale is not (`I-UX-7`).
	 */
	const notFound = $derived(page.status === 404);
</script>

<ErrorState
	state={{
		kind: 'error',
		summary: notFound ? t('page.notFound.title') : t('page.failed.title'),
		consequence: notFound ? t('page.notFound.body') : t('page.failed.body'),
	}}
/>
