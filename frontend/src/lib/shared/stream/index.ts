// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * The one live connection, and the indicator that says when it is not.
 *
 * A lost connection degrades liveness, never correctness: every surface the
 * stream feeds is correct after a plain page load with no stream at all
 * (PRD §9, `I-UX-9`).
 */

import DisconnectionIndicator from './disconnection-indicator.svelte';

export type { StreamStatus } from './backoff';
export { BASE_DELAY_MS, backoffDelayMs, MAX_DELAY_MS } from './backoff';
export type { TopicHandler } from './connection.svelte';
export { StreamConnection } from './connection.svelte';
export { DisconnectionIndicator };
