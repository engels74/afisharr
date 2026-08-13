// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * The watchdog half of `connection.svelte.ts`, split from `connection.test.ts`.
 *
 * These are the only cases in that suite that turn on wall-clock timing, and
 * they are the only ones that need the polling helpers below. Kept together so
 * the timing-sensitive tests are one file a reader can find and re-run when a
 * heartbeat or backoff constant moves.
 */

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

/**
 * Waits until a connection past `previous` has been constructed.
 *
 * Polled rather than slept, because the delay is not a constant: the watchdog
 * is derived from the server's stated heartbeat and the retry is jittered, so
 * a fixed sleep is either flaky or long enough for the *next* timer to fire
 * inside it and move the thing being asserted about.
 */
async function reconnected(previous: number): Promise<void> {
	await until(
		() => FakeEventSource.instances.length > previous,
		`a connection past ${previous}`,
	);
}

/** Waits for `settled`, or fails saying what never happened. */
async function until(
	settled: () => boolean,
	what = 'the expected state',
): Promise<void> {
	const deadline = Date.now() + 2000;
	while (!settled()) {
		if (Date.now() > deadline) {
			throw new Error(`${what} never arrived`);
		}
		await Bun.sleep(10);
	}
}

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
		// Answered as soon as it appears, which is what a real connection does:
		// the server writes its opening event on accept. Waited out on a fixed
		// sleep instead, the replacement is itself a connection carrying
		// nothing, and the watchdog that now covers the connecting phase takes
		// it down too — correctly, and before the assertions below could look.
		await reconnected(opened);

		expect(FakeEventSource.instances.length).toBeGreaterThan(opened);
		expect(latest().closed).toBe(false);
		latest().emit('open');
		expect(stream.status).toBe('live');
		expect(refetches).toBe(1);
		stream.close();
	});

	test('a connection that never opens is torn down rather than left hanging', async () => {
		// The half-open case one step earlier, and the one nothing covered: a
		// socket the instance accepts and then says nothing on fires neither
		// `open` nor `error`, so a watchdog armed only from those listeners is
		// never armed at all. The status stays 'connecting', which the
		// indicator renders as nothing — a shell with no live updates and
		// nothing on screen saying so, for as long as the proxy holds the
		// socket (`I-UX-9`).
		const stream = new StreamConnection();
		stream.open();
		latest().emit('open');
		latest().emit('stream', { heartbeatSeconds: 0.01, topics: ['jobs'] });

		// The replacement this drives is never answered on, so nothing but a
		// watchdog covering the connecting phase can end it.
		const opened = FakeEventSource.instances.length;
		await reconnected(opened);
		const silent = latest();
		expect(silent.closed).toBe(false);

		await until(() => silent.closed);
		expect(silent.closed).toBe(true);
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
