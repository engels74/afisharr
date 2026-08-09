// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { defineConfig, presetWind4, transformerVariantGroup } from 'unocss';

// presetWind4 is the Tailwind-v4-compatible preset and the only one this
// project uses: presetUno and presetWind3 are the superseded names, and the
// shadcn-svelte components are authored against v4 class names and variables.
export default defineConfig({
	presets: [presetWind4({ preflights: { reset: true } })],
	transformers: [transformerVariantGroup()],
});
