// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * A stand-in for the browser's `EventSource`, for the connection's tests.
 *
 * The connection's contract is about what it does when the stream stops, lags,
 * and comes back — none of which a real server can be asked to do on demand.
 *
 * A module of its own rather than a block at the top of the test file: it is
 * the harness two suites drive, and a copy in each would be two fakes with one
 * name, free to diverge in exactly the behaviour the tests are asserting about
 * (P7). Nothing in the application imports it, so it is not in the bundle.
 */
export class FakeEventSource {
	/** Every fake constructed since the last reset, oldest first. */
	static instances: FakeEventSource[] = [];

	readonly url: string;
	#listeners = new Map<string, Set<(event: Event) => void>>();
	closed = false;

	constructor(url: string) {
		this.url = url;
		FakeEventSource.instances.push(this);
	}

	addEventListener(type: string, listener: (event: Event) => void): void {
		const set = this.#listeners.get(type) ?? new Set();
		set.add(listener);
		this.#listeners.set(type, set);
	}

	close(): void {
		this.closed = true;
	}

	/** Drives an event, as the server would. */
	emit(type: string, data?: unknown): void {
		const event =
			data === undefined
				? new Event(type)
				: new MessageEvent(type, { data: JSON.stringify(data) });
		for (const listener of this.#listeners.get(type) ?? []) {
			listener(event);
		}
	}
}

/** The most recently constructed fake. */
export function latest(): FakeEventSource {
	const source = FakeEventSource.instances.at(-1);
	if (!source) {
		throw new Error('no connection was opened');
	}
	return source;
}

/** The real global, so a suite can put it back when it is done. */
const realEventSource = globalThis.EventSource;

/** Puts the fake in place of the browser's global, and forgets past instances. */
export function installFakeEventSource(): void {
	FakeEventSource.instances = [];
	// biome-ignore lint/suspicious/noExplicitAny: substituting a browser global
	(globalThis as any).EventSource = FakeEventSource;
}

/** Puts the browser's global back. */
export function restoreEventSource(): void {
	// biome-ignore lint/suspicious/noExplicitAny: restoring a browser global
	(globalThis as any).EventSource = realEventSource;
}
