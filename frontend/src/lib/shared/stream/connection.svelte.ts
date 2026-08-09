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

	/** Opens the connection. Safe to call again; a second call is a no-op. */
	open(url = '/api/stream'): void {
		if (this.#source || this.#stopped) {
			return;
		}
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
			this.#dropAndRetry(url);
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
	}

	/** Closes the connection and stops reconnecting. */
	close(): void {
		this.#stopped = true;
		this.#clearTimers();
		this.#source?.close();
		this.#source = undefined;
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
	 * constant here, so the two cannot drift apart.
	 */
	#armWatchdog(): void {
		clearTimeout(this.#watchdog);
		this.#watchdog = setTimeout(() => {
			this.status = 'disconnected';
		}, watchdogDelayMs(this.#heartbeatMs));
	}

	#dropAndRetry(url: string): void {
		this.#clearTimers();
		this.#source?.close();
		this.#source = undefined;
		if (this.#stopped) {
			return;
		}
		this.status = 'disconnected';
		this.#attempt += 1;
		this.#retry = setTimeout(() => {
			this.open(url);
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
