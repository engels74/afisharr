<!--
	SPDX-FileCopyrightText: 2026 Afisharr contributors
	SPDX-License-Identifier: AGPL-3.0-or-later
-->
<script lang="ts">
	// The interface shell is Phase 1. This route exists so the static build has
	// an entry point to prerender, and it holds no product copy: every
	// user-visible string goes through the i18n catalogue from Phase 1 onward
	// (I-UX-7), and a hard-coded one here would be the first exception.
</script>

<div class="min-h-screen grid place-items-center">
	<p class="text-sm text-[var(--muted-foreground)]" data-testid="shell-placeholder">afisharr</p>
</div>
