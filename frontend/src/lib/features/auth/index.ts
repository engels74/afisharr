// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/** Signing in: local credentials, and the plex.tv PIN and OAuth flows. */

import LoginForm from './login-form.svelte';
import NotPermittedPanel from './not-permitted-panel.svelte';
import PlexPinPanel from './plex-pin-panel.svelte';
import SignOutButton from './sign-out-button.svelte';

export type { AuthResult, PinStarted, PinState, SignedIn } from './auth-client';
export {
	pollPlexPin,
	readSession,
	signIn,
	signOut,
	startPlexPin,
} from './auth-client';
export type { Session, SessionState } from './session.svelte';
export { createSession, session } from './session.svelte';
export { LoginForm, NotPermittedPanel, PlexPinPanel, SignOutButton };
