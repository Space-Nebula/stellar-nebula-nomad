/** A contract event as delivered to streaming clients. */
export interface ContractEvent {
  /** RPC event id — also the resume cursor. */
  id: string;
  /** Contract that emitted the event. */
  contractId: string;
  /** Decoded topic strings (XDR topics are passed through verbatim). */
  topics: string[];
  /** Event payload (XDR value passed through verbatim). */
  value: unknown;
  /** Ledger sequence the event was included in. */
  ledger: number;
  /** Ledger close time, ISO-8601. */
  ledgerClosedAt: string;
}

/** Messages clients send over the WebSocket. */
export type ClientMessage =
  | {
      type: "subscribe";
      /** Topic prefixes to match; empty/omitted means all events. */
      topics?: string[];
      /** Restrict to specific contract ids; empty/omitted means all. */
      contractIds?: string[];
    }
  | { type: "unsubscribe" }
  | { type: "ping" };

/** Messages the server sends to clients. */
export type ServerMessage =
  | { type: "connected"; clientId: string }
  | { type: "subscribed"; topics: string[]; contractIds: string[] }
  | { type: "unsubscribed" }
  | { type: "event"; event: ContractEvent }
  | { type: "pong" }
  | { type: "error"; message: string };

/** Minimal structural WebSocket used by the connection manager (test-injectable). */
export interface StreamSocket {
  readyState: number;
  send(data: string): void;
  ping(): void;
  terminate(): void;
  close(code?: number, reason?: string): void;
  on(event: "message", listener: (data: unknown) => void): void;
  on(event: "close", listener: () => void): void;
  on(event: "pong", listener: () => void): void;
  on(event: "error", listener: (error: Error) => void): void;
}

/** ws readyState OPEN — mirrored here so tests don't need the ws package. */
export const SOCKET_OPEN = 1;
