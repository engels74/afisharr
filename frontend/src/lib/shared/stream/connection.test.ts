// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { StreamConnection } from './connection.svelte';
import {
	FakeEventSource,
	installFakeEventSource,
	latest,
	restoreEventSource,
} from './fake-event-source';

beforeEach(installFakeEventSource);
afterEach(restoreEventSource);

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

	test('a half-open connection is torn down and reconnected', async () => {
		// A connection that stops carrying events without erroring fires no
		// `error` at all. Relabelling the status and leaving the `EventSource`
		// in place makes `open()` a no-op forever, so nothing reconnects and no
		// refetcher ever runs.
		const stream = new StreamConnection();
		let refetches = 0;
		stream.onreconnect(() => {
			refetches += 1;
		});
		stream.open();
		latest().emit('open');
		latest().emit('stream', { heartbeatSeconds: 0.01, topics: ['jobs'] });

		const opened = FakeEventSource.instances.length;
		// The watchdog fires at 260ms; the first backoff is at most a second.
		await Bun.sleep(1500);

		expect(FakeEventSource.instances.length).toBeGreaterThan(opened);
		expect(latest().closed).toBe(false);
		latest().emit('open');
		expect(stream.status).toBe('live');
		expect(refetches).toBe(1);
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
	test('a closed connection stops reconnecting on its own', async () => {
		const stream = new StreamConnection();
		stream.open();
		latest().emit('open');
		latest().emit('error');
		stream.close();

		expect(stream.status).toBe('disconnected');
		const opened = FakeEventSource.instances.length;
		// Long enough for the pending backoff retry to have fired.
		await Bun.sleep(1500);
		expect(FakeEventSource.instances.length).toBe(opened);
	});

	test('a closed connection reopens when it is asked to', () => {
		// The root layout keeps one connection and closes it whenever it
		// navigates into the login or setup journey. Signing out and back in
		// arrives at `open()` on an object that has already been stopped, and a
		// `close()` that could not be undone would leave a permanently dead
		// stream with nothing to explain it.
		const stream = new StreamConnection();
		stream.open();
		latest().emit('open');
		stream.close();

		const opened = FakeEventSource.instances.length;
		stream.open();
		expect(FakeEventSource.instances.length).toBe(opened + 1);
		latest().emit('open');
		expect(stream.status).toBe('live');
		stream.close();
	});

	test('a reopened connection is a first attempt and not a continued one', async () => {
		// The failures of the last visit say nothing about this one. A counter
		// left standing across `close()` labelled this `reconnecting` on a
		// connection that had never failed, and made the first drop after it
		// wait out a fully backed-off delay (`I-UX-9`).
		const stream = new StreamConnection();
		stream.open();
		latest().emit('open');
		for (let drop = 0; drop < 5; drop += 1) {
			latest().emit('error');
		}
		stream.close();

		stream.open();
		expect(stream.status).toBe('connecting');
		latest().emit('open');
		const opened = FakeEventSource.instances.length;
		latest().emit('error');

		// A first attempt retries inside a second; a sixth waits 16 to 32.
		await Bun.sleep(1500);
		expect(FakeEventSource.instances.length).toBeGreaterThan(opened);
		stream.close();
	});
});
