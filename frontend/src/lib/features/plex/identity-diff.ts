// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * How much of two machine identifiers is identical, counting from the left.
 *
 * Pure, and its own module so it is unit-testable without a browser: what it
 * decides is where the interface stops dimming and starts emphasising, and a
 * wrong answer there points the operator at the wrong characters of the one
 * comparison `I-ID-5` is about.
 *
 * Counted in code points rather than UTF-16 units, so a split never lands
 * inside a surrogate pair and renders a replacement character. Plex identifiers
 * are hexadecimal today; the identifier space is somebody else's to change.
 */
export function sharedPrefix(
	one: string | null | undefined,
	other: string | null | undefined,
): number {
	if (!one || !other) {
		return 0;
	}
	const left = [...one];
	const right = [...other];
	let shared = 0;
	while (
		shared < left.length &&
		shared < right.length &&
		left[shared] === right[shared]
	) {
		shared += 1;
	}
	// Measured back in UTF-16 units, because that is what `String.slice` takes.
	return left.slice(0, shared).join('').length;
}
