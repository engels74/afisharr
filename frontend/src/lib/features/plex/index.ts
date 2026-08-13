// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/** The Plex connection: whether the bound server is the server that answers. */

import ConnectionPanel from './connection-panel.svelte';

export type {
	ConnectionResult,
	PlexConnection,
	PlexConnectionState,
} from './plex-client';
export { blocks, checkConnection } from './plex-client';
export { ConnectionPanel };
