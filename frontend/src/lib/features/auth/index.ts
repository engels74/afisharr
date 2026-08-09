// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/** Signing in: local credentials, and the plex.tv PIN and OAuth flows. */

import LoginForm from './login-form.svelte';
import PlexPinPanel from './plex-pin-panel.svelte';

export type { AuthResult, PinStarted, PinState, SignedIn } from './auth-client';
export {
	pollPlexPin,
	readSession,
	signIn,
	signOut,
	startPlexPin,
} from './auth-client';
export type { SessionState } from './session.svelte';
export { Session } from './session.svelte';
export { LoginForm, PlexPinPanel };
