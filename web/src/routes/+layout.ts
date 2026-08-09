// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * Prerender everything and render nothing on a server.
 *
 * The build output is a static bundle embedded in the Rust binary, and every
 * read and write goes through the generated OpenAPI client called client-side
 * (PRD §24.4). A server load function would have nowhere to run.
 */
export const prerender = true;
export const ssr = false;
