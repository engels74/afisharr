// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

// `bun test` covers pure `.ts` and `.svelte.ts` modules; some of them touch DOM
// APIs, so a DOM is registered before any test file loads. Component rendering
// belongs to Vitest browser mode, not here.
import { GlobalRegistrator } from '@happy-dom/global-registrator';

GlobalRegistrator.register();
