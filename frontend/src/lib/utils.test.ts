// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test';
import { cn } from './utils';

// `cn` is the seam between two libraries that have to agree: the copied
// shadcn-svelte components are authored with Tailwind class names, and
// `tailwind-merge` resolves conflicts among them, while UnoCSS `presetWind4`
// is what actually emits the CSS. These assert the behaviour every
// `class={cn(variants(), className)}` call depends on, so a merger that stops
// recognising the utilities this project generates fails here rather than as a
// component that quietly ignores its `class` prop.

test('a later utility wins over an earlier one in the same group', () => {
	expect(cn('px-2', 'px-4')).toBe('px-4');
});

test('utilities from different groups are both kept', () => {
	expect(cn('px-4', 'text-sm')).toBe('px-4 text-sm');
});

test('falsy and conditional values drop out', () => {
	expect(
		cn('rounded', false && 'hidden', undefined, null, { hidden: false }),
	).toBe('rounded');
});

test('a caller class overrides the variant it is merged after', () => {
	expect(cn('bg-primary text-sm', 'bg-destructive')).toBe(
		'text-sm bg-destructive',
	);
});

test('variant modifiers are compared within their own group', () => {
	expect(cn('hover:px-2', 'px-4')).toBe('hover:px-2 px-4');
});
