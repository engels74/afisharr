// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/** First run: the claim, the administrator, and the step the server derives. */

import AdminForm from './admin-form.svelte';
import ClaimForm from './claim-form.svelte';

export type { SetupResult, SetupStatus } from './setup-client';
export { claim, createAdmin, readStatus, recover } from './setup-client';
export { AdminForm, ClaimForm };
