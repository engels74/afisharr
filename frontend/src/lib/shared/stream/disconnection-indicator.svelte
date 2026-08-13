<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	import type { ClassValue } from 'svelte/elements';
	import { t } from '$lib/shared/i18n';
	import { cn } from '$lib/utils';
	import type { StreamStatus } from './backoff';

	interface Props {
		status: StreamStatus;
		class?: ClassValue;
	}

	let { status, class: className }: Props = $props();
</script>

<!--
	Small and non-modal, and distinct from every other state (PRD §9). Silently
	showing frozen numbers as live is how an operator learns to distrust the
	whole interface, so this appears within one missed heartbeat and says only
	what it knows.
-->
{#if status === 'disconnected' || status === 'reconnecting'}
	<p
		class={cn('text-xs text-[var(--muted-foreground)]', className)}
		data-slot="disconnection-indicator"
		data-status={status}
		role="status"
		aria-live="polite"
	>
		{status === 'reconnecting' ? t('stream.reconnecting') : t('stream.disconnected')}
	</p>
{/if}
