// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { StreamConnection } from './connection.svelte';

/**
 * A stand-in for the browser's `EventSource`.
 *
 * The connection's contract is about what it does when the stream stops, lags,
 * and comes back — none of which a real server can be asked to do on demand.
 */
class FakeEventSource {
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

const realEventSource = globalThis.EventSource;

beforeEach(() => {
	FakeEventSource.instances = [];
	// biome-ignore lint/suspicious/noExplicitAny: substituting a browser global
	(globalThis as any).EventSource = FakeEventSource;
});

afterEach(() => {
	// biome-ignore lint/suspicious/noExplicitAny: restoring a browser global
	(globalThis as any).EventSource = realEventSource;
});

/** The most recently constructed fake. */
function latest(): FakeEventSource {
	const source = FakeEventSource.instances.at(-1);
	if (!source) {
		throw new Error('no connection was opened');
	}
	return source;
}

describe('opening', () => {
	test('a fresh connection reports connecting until the stream opens', () => {
		const stream = new StreamConnection();
		stream.open();
		expect(stream.status).toBe('connecting');

		latest().emit('open');
		expect(stream.status).toBe('live');
		stream.close();
	});

	test('a second open is a no-op rather than a second connection', () => {
		const stream = new StreamConnection();
		stream.open();
		stream.open();
		expect(FakeEventSource.instances.length).toBe(1);
		stream.close();
	});
});

describe('topics', () => {
	test('a subscriber receives its own topic and not another', () => {
		const stream = new StreamConnection();
		stream.open();
		latest().emit('open');

		const jobs: unknown[] = [];
		const sources: unknown[] = [];
		stream.on('jobs', (payload) => jobs.push(payload));
		stream.on('sources', (payload) => sources.push(payload));

		latest().emit('jobs', { runId: '01J' });
		expect(jobs).toEqual([{ runId: '01J' }]);
		expect(sources).toEqual([]);
		stream.close();
	});

	test('an unsubscribed handler stops receiving', () => {
		const stream = new StreamConnection();
		stream.open();
		latest().emit('open');

		const seen: unknown[] = [];
		const stop = stream.on('jobs', (payload) => seen.push(payload));
		latest().emit('jobs', { runId: 'first' });
		stop();
		latest().emit('jobs', { runId: 'second' });

		expect(seen).toEqual([{ runId: 'first' }]);
		stream.close();
	});

	test('a payload this build cannot read does not tear the stream down', () => {
		const stream = new StreamConnection();
		stream.open();
		latest().emit('open');

		const seen: unknown[] = [];
		stream.on('jobs', (payload) => seen.push(payload));
		// A raw MessageEvent whose data is not JSON.
		latest().emit('jobs', undefined);

		expect(stream.status).toBe('live');
		stream.close();
	});
});

describe('reconnection', () => {
	test('a reconnect refetches rather than replaying', () => {
		// `I-UX-9`: a client that missed events during a disconnect and then
		// reconnects ends up identical to one that loaded the page fresh. The
		// mechanism is a refetch, and there is no event buffer to replay from.
		const stream = new StreamConnection();
		let refetches = 0;
		stream.onreconnect(() => {
			refetches += 1;
		});

		stream.open();
		latest().emit('open');
		expect(refetches).toBe(0);

		// The connection drops and comes back.
		latest().emit('error');
		expect(stream.status).toBe('disconnected');

		stream.open();
		latest().emit('open');
		expect(refetches).toBe(1);
		stream.close();
	});

	test('being told the stream lagged triggers the same refetch', () => {
		const stream = new StreamConnection();
		let refetches = 0;
		stream.onreconnect(() => {
			refetches += 1;
		});
		stream.open();
		latest().emit('open');

		latest().emit('stream', { lagged: true });
		expect(refetches).toBe(1);
		stream.close();
	});

	test('an unsubscribed refetch is not run', () => {
		const stream = new StreamConnection();
		let refetches = 0;
		const stop = stream.onreconnect(() => {
			refetches += 1;
		});
		stop();

		stream.open();
		latest().emit('open');
		latest().emit('error');
		stream.open();
		latest().emit('open');

		expect(refetches).toBe(0);
		stream.close();
	});
});

describe('the disconnection watchdog', () => {
	test('the heartbeat interval comes from the server, not from a constant', async () => {
		// One missed heartbeat and the indicator appears. The interval is the
		// server's, so the two sides cannot drift apart.
		const stream = new StreamConnection();
		stream.open();
		latest().emit('open');
		latest().emit('stream', { heartbeatSeconds: 0.01, topics: ['jobs'] });
		expect(stream.status).toBe('live');

		// 10ms interval, so the watchdog fires at 260ms.
		await Bun.sleep(400);
		expect(stream.status).toBe('disconnected');
		stream.close();
	});

	test('an arriving event rearms the watchdog', async () => {
		const stream = new StreamConnection();
		stream.open();
		latest().emit('open');
		latest().emit('stream', { heartbeatSeconds: 0.5, topics: ['jobs'] });

		// Beating well inside the 600ms watchdog keeps it live.
		for (let beat = 0; beat < 4; beat += 1) {
			await Bun.sleep(100);
			latest().emit('jobs', { beat });
		}
		expect(stream.status).toBe('live');
		stream.close();
	});
});

describe('closing', () => {
	test('a closed connection stops reconnecting', () => {
		const stream = new StreamConnection();
		stream.open();
		latest().emit('open');
		stream.close();

		expect(stream.status).toBe('disconnected');
		const opened = FakeEventSource.instances.length;
		stream.open();
		expect(FakeEventSource.instances.length).toBe(opened);
	});
});
