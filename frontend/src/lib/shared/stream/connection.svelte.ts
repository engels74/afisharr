// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import { backoffDelayMs, type StreamStatus, watchdogDelayMs } from './backoff';

/** A handler for one topic's events. */
export type TopicHandler = (payload: unknown) => void;

/** What the server says when the connection opens. */
interface StreamOpened {
	heartbeatSeconds: number;
	topics: string[];
}

/**
 * The one multiplexed connection.
 *
 * The stream is an accelerator and never a source of truth: every surface it
 * feeds is correct after a plain page load with no stream at all, and a
 * reconnect refetches rather than replaying (PRD §9, `I-UX-9`). That is why
 * `onreconnect` exists and there is no event buffer — a client that missed
 * events refetches, which is exactly what a fresh load does.
 */
export class StreamConnection {
	/** Whether the stream is carrying events right now. */
	status = $state<StreamStatus>('connecting');

	#source: EventSource | undefined;
	#handlers = new Map<string, Set<TopicHandler>>();
	#refetchers = new Set<() => void>();
	#attempt = 0;
	#watchdog: ReturnType<typeof setTimeout> | undefined;
	#retry: ReturnType<typeof setTimeout> | undefined;
	#heartbeatMs = 15_000;
	#stopped = false;
	#url = '/api/stream';

	/** Subscribes `handler` to `topic`, and returns the unsubscribe. */
	on(topic: string, handler: TopicHandler): () => void {
		const handlers = this.#handlers.get(topic) ?? new Set();
		handlers.add(handler);
		this.#handlers.set(topic, handlers);
		return () => {
			handlers.delete(handler);
		};
	}

	/**
	 * Registers a refetch to run whenever the stream reconnects.
	 *
	 * This is the whole reconciliation strategy. A client that reconnects has
	 * missed events and knows it; replaying them would be a second path to the
	 * same state, and the one exercised less would be the one that is wrong.
	 */
	onreconnect(refetch: () => void): () => void {
		this.#refetchers.add(refetch);
		return () => {
			this.#refetchers.delete(refetch);
		};
	}

	/**
	 * Opens the connection.
	 *
	 * Safe to call again while one is live; that call is a no-op. Calling it
	 * after `close()` reopens: the root layout keeps one connection for the
	 * life of the tab and closes it whenever it navigates into the login or
	 * setup journey, so signing out and back in arrives here on an object that
	 * has already been stopped. A `close()` that could not be undone would make
	 * that a permanently dead stream with no error anywhere to explain it.
	 */
	open(url = this.#url): void {
		this.#url = url;
		if (this.#source) {
			return;
		}
		this.#stopped = false;
		this.status = this.#attempt === 0 ? 'connecting' : 'reconnecting';

		const source = new EventSource(url, { withCredentials: true });
		this.#source = source;

		source.addEventListener('open', () => {
			const reconnected = this.#attempt > 0;
			this.#attempt = 0;
			this.status = 'live';
			this.#armWatchdog();
			if (reconnected) {
				for (const refetch of this.#refetchers) {
					refetch();
				}
			}
		});

		source.addEventListener('error', () => {
			this.#dropAndRetry();
		});

		// The server's own topic carries the opening event and the lag notice.
		source.addEventListener('stream', (event) => {
			this.#armWatchdog();
			const payload = parse((event as MessageEvent<string>).data);
			this.#adoptHeartbeat(payload);
			if (isLagged(payload)) {
				// Being told the stream lagged is the same instruction as a
				// reconnect: refetch, because the events are gone.
				for (const refetch of this.#refetchers) {
					refetch();
				}
			}
			this.#dispatch('stream', payload);
		});

		for (const topic of ['jobs', 'sources']) {
			source.addEventListener(topic, (event) => {
				this.#armWatchdog();
				this.#dispatch(topic, parse((event as MessageEvent<string>).data));
			});
		}

		// Armed on the attempt, not only once it succeeds. A connection that
		// never reaches OPEN fires neither `open` nor `error` — a proxy with a
		// long `proxy_read_timeout`, which is the usual SSE configuration,
		// holds the socket while the instance behind it says nothing — so
		// without this there was no timer on it at all. `status` stayed
		// 'connecting', which the indicator renders as nothing, and the
		// operator had a shell with no live updates and nothing on screen
		// saying so for as long as the proxy held the socket (`I-UX-9`).
		this.#armWatchdog();
	}

	/**
	 * Closes the connection and stops reconnecting.
	 *
	 * The attempt count goes with it. `close()` and `open()` bracket a whole
	 * visit to the shell, so the failures of the last one say nothing about the
	 * next: a counter left standing made `open()` label a brand-new connection
	 * `reconnecting`, and made the first drop after it wait out a fully
	 * backed-off delay — half a minute of dead stream after a one-second blip
	 * (`I-UX-9`).
	 */
	close(): void {
		this.#stopped = true;
		this.#clearTimers();
		this.#source?.close();
		this.#source = undefined;
		this.#attempt = 0;
		this.status = 'disconnected';
	}

	#dispatch(topic: string, payload: unknown): void {
		for (const handler of this.#handlers.get(topic) ?? []) {
			handler(payload);
		}
	}

	#adoptHeartbeat(payload: unknown): void {
		const opened = payload as Partial<StreamOpened> | null;
		if (opened && typeof opened.heartbeatSeconds === 'number') {
			this.#heartbeatMs = opened.heartbeatSeconds * 1000;
			this.#armWatchdog();
		}
	}

	/**
	 * Restarts the "one missed heartbeat" timer.
	 *
	 * The interval comes from the server's opening event rather than from a
	 * constant here, so the two cannot drift apart. Firing it tears the
	 * connection down and reconnects rather than only relabelling the status: a
	 * half-open connection produces no `error` event at all, so leaving the
	 * `EventSource` in place makes `open()` a no-op forever and neither the
	 * backoff nor the registered refetchers ever run. The indicator would say
	 * "disconnected" for as long as the tab is open and nothing would try to
	 * fix it (`I-UX-9`).
	 */
	#armWatchdog(): void {
		clearTimeout(this.#watchdog);
		this.#watchdog = setTimeout(() => {
			this.#dropAndRetry();
		}, watchdogDelayMs(this.#heartbeatMs));
	}

	#dropAndRetry(): void {
		this.#clearTimers();
		this.#source?.close();
		this.#source = undefined;
		if (this.#stopped) {
			return;
		}
		this.status = 'disconnected';
		this.#attempt += 1;
		this.#retry = setTimeout(() => {
			this.open();
		}, backoffDelayMs(this.#attempt));
	}

	#clearTimers(): void {
		clearTimeout(this.#watchdog);
		clearTimeout(this.#retry);
	}
}

function parse(data: string): unknown {
	try {
		return JSON.parse(data);
	} catch {
		// A payload this build cannot read is not a reason to tear the stream
		// down: the surfaces it feeds are correct without it.
		return null;
	}
}

function isLagged(payload: unknown): boolean {
	return (payload as { lagged?: boolean } | null)?.lagged === true;
}
