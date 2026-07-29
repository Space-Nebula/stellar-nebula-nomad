import axios from "axios";
import { ContractEvent } from "./types";

export interface EventStreamerOptions {
  /** Soroban RPC endpoint. */
  rpcUrl: string;
  /** Contract ids to stream events for. */
  contractIds: string[];
  /** Poll interval while healthy, ms. */
  pollIntervalMs?: number;
  /** Initial reconnection backoff after an RPC failure, ms. */
  initialBackoffMs?: number;
  /** Backoff ceiling, ms. */
  maxBackoffMs?: number;
  /** Receives every new event, in ledger order. */
  onEvent: (event: ContractEvent) => void;
  /** Observability hook for RPC failures / reconnection attempts. */
  onError?: (error: Error, consecutiveFailures: number) => void;
}

interface RawRpcEvent {
  id: string;
  contractId: string;
  topic?: string[];
  value?: unknown;
  ledger: number;
  ledgerClosedAt: string;
}

const DEFAULT_POLL_INTERVAL_MS = 2_000;
const DEFAULT_INITIAL_BACKOFF_MS = 1_000;
const DEFAULT_MAX_BACKOFF_MS = 60_000;

/**
 * Streams contract events from Soroban RPC's `getEvents` into a callback.
 *
 * The RPC connection is a poll loop with cursor-based resumption: each
 * response's pagination cursor is carried into the next request, so no
 * events are skipped or duplicated across polls. On RPC failure the loop
 * stays alive and reconnects with exponential backoff (doubling from
 * `initialBackoffMs` up to `maxBackoffMs`), resuming from the last
 * successful cursor once the endpoint recovers.
 */
export class EventStreamer {
  private readonly options: Required<
    Pick<
      EventStreamerOptions,
      "pollIntervalMs" | "initialBackoffMs" | "maxBackoffMs"
    >
  > &
    EventStreamerOptions;

  private cursor: string | null = null;
  private startLedger: number | null = null;
  private running = false;
  private timer: NodeJS.Timeout | null = null;
  private consecutiveFailures = 0;

  constructor(options: EventStreamerOptions) {
    this.options = {
      pollIntervalMs: DEFAULT_POLL_INTERVAL_MS,
      initialBackoffMs: DEFAULT_INITIAL_BACKOFF_MS,
      maxBackoffMs: DEFAULT_MAX_BACKOFF_MS,
      ...options,
    };
  }

  /** Starts streaming from the given ledger (defaults to the current tip). */
  async start(startLedger?: number): Promise<void> {
    if (this.running) return;
    this.running = true;
    this.startLedger = startLedger ?? null;
    await this.poll();
  }

  stop(): void {
    this.running = false;
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }

  get isRunning(): boolean {
    return this.running;
  }

  /** Last processed event id — persist and pass back in on restart. */
  get lastCursor(): string | null {
    return this.cursor;
  }

  /** Current backoff delay in ms — exposed for tests and metrics. */
  currentBackoffMs(): number {
    if (this.consecutiveFailures === 0) return this.options.pollIntervalMs;
    const backoff =
      this.options.initialBackoffMs *
      Math.pow(2, this.consecutiveFailures - 1);
    return Math.min(backoff, this.options.maxBackoffMs);
  }

  private async poll(): Promise<void> {
    if (!this.running) return;
    try {
      await this.fetchEvents();
      this.consecutiveFailures = 0;
    } catch (error) {
      this.consecutiveFailures += 1;
      if (this.options.onError) {
        this.options.onError(
          error instanceof Error ? error : new Error(String(error)),
          this.consecutiveFailures,
        );
      }
    }
    if (!this.running) return;
    this.timer = setTimeout(() => {
      void this.poll();
    }, this.currentBackoffMs());
  }

  private async fetchEvents(): Promise<void> {
    const params: Record<string, unknown> = {
      filters: [
        {
          type: "contract",
          contractIds: this.options.contractIds,
        },
      ],
      pagination: { limit: 100 },
    };
    if (this.cursor) {
      (params.pagination as Record<string, unknown>).cursor = this.cursor;
    } else if (this.startLedger !== null) {
      params.startLedger = this.startLedger;
    } else {
      // First poll with no cursor: anchor at the current tip so we stream
      // new events rather than replaying history.
      params.startLedger = await this.fetchLatestLedger();
    }

    const response = await axios.post(this.options.rpcUrl, {
      jsonrpc: "2.0",
      id: 1,
      method: "getEvents",
      params,
    });

    const result = response.data?.result;
    if (!result) {
      throw new Error("Malformed getEvents response: missing result");
    }

    const events: RawRpcEvent[] = result.events ?? [];
    for (const raw of events) {
      const event: ContractEvent = {
        id: raw.id,
        contractId: raw.contractId,
        topics: raw.topic ?? [],
        value: raw.value ?? null,
        ledger: raw.ledger,
        ledgerClosedAt: raw.ledgerClosedAt,
      };
      this.cursor = raw.id;
      this.options.onEvent(event);
    }

    if (typeof result.cursor === "string" && result.cursor) {
      this.cursor = result.cursor;
    }
  }

  private async fetchLatestLedger(): Promise<number> {
    const response = await axios.post(this.options.rpcUrl, {
      jsonrpc: "2.0",
      id: 1,
      method: "getLatestLedger",
      params: {},
    });
    const sequence = response.data?.result?.sequence;
    if (typeof sequence !== "number") {
      throw new Error("Malformed getLatestLedger response: missing sequence");
    }
    return sequence;
  }
}
