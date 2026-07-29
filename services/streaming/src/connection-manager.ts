import {
  ClientMessage,
  ContractEvent,
  ServerMessage,
  SOCKET_OPEN,
  StreamSocket,
} from "./types";

export interface ConnectionManagerOptions {
  /** Heartbeat ping interval, ms. Sockets missing a pong are terminated. */
  heartbeatIntervalMs?: number;
  /** Maximum concurrent client connections; further sockets are refused. */
  maxConnections?: number;
  /** Observability hook: connection count changes. */
  onConnectionCountChange?: (count: number) => void;
}

interface ClientState {
  socket: StreamSocket;
  /** Topic prefixes the client subscribed to; null = not subscribed. */
  topics: string[] | null;
  contractIds: string[] | null;
  alive: boolean;
}

const DEFAULT_HEARTBEAT_INTERVAL_MS = 30_000;
const DEFAULT_MAX_CONNECTIONS = 10_000;

let nextClientId = 0;

/**
 * Tracks WebSocket clients and their event subscriptions.
 *
 * Connection management:
 *  - a connection cap, refusing sockets above it with close code 1013
 *    ("try again later") so clients know to back off before reconnecting;
 *  - liveness heartbeats — each interval every socket is pinged and any
 *    socket that never answered the previous ping is terminated, so
 *    half-open TCP connections (mobile clients dropping off networks)
 *    don't accumulate;
 *  - per-client subscription filters (topic prefixes and contract ids)
 *    applied on broadcast.
 *
 * Client-side reconnection is documented in the service README: clients
 * resubscribe on reconnect and the server replays nothing — durable
 * delivery belongs to the webhook service; this stream is for live UX.
 */
export class ConnectionManager {
  private readonly heartbeatIntervalMs: number;
  private readonly maxConnections: number;
  private readonly onConnectionCountChange?: (count: number) => void;
  private readonly clients = new Map<string, ClientState>();
  private heartbeatTimer: NodeJS.Timeout | null = null;

  constructor(options: ConnectionManagerOptions = {}) {
    this.heartbeatIntervalMs =
      options.heartbeatIntervalMs ?? DEFAULT_HEARTBEAT_INTERVAL_MS;
    this.maxConnections = options.maxConnections ?? DEFAULT_MAX_CONNECTIONS;
    this.onConnectionCountChange = options.onConnectionCountChange;
  }

  get connectionCount(): number {
    return this.clients.size;
  }

  /** Starts the heartbeat loop. Idempotent. */
  start(): void {
    if (this.heartbeatTimer) return;
    this.heartbeatTimer = setInterval(() => {
      this.runHeartbeat();
    }, this.heartbeatIntervalMs);
  }

  /** Stops heartbeats and closes every client socket. */
  stop(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
    for (const [clientId, client] of this.clients) {
      client.socket.close(1001, "server shutting down");
      this.clients.delete(clientId);
    }
    this.notifyCount();
  }

  /**
   * Registers a newly accepted socket. Returns the client id, or null when
   * the connection cap is hit (the socket is refused and closed).
   */
  register(socket: StreamSocket): string | null {
    if (this.clients.size >= this.maxConnections) {
      socket.close(1013, "connection limit reached, try again later");
      return null;
    }

    const clientId = `client-${++nextClientId}`;
    const client: ClientState = {
      socket,
      topics: null,
      contractIds: null,
      alive: true,
    };
    this.clients.set(clientId, client);

    socket.on("pong", () => {
      client.alive = true;
    });
    socket.on("close", () => {
      this.clients.delete(clientId);
      this.notifyCount();
    });
    socket.on("error", () => {
      socket.terminate();
      this.clients.delete(clientId);
      this.notifyCount();
    });
    socket.on("message", (data) => {
      this.handleMessage(clientId, data);
    });

    this.send(socket, { type: "connected", clientId });
    this.notifyCount();
    return clientId;
  }

  /** Delivers an event to every subscribed client whose filters match. */
  broadcast(event: ContractEvent): number {
    let delivered = 0;
    for (const client of this.clients.values()) {
      if (client.topics === null) continue; // not subscribed
      if (!this.matches(client, event)) continue;
      if (client.socket.readyState !== SOCKET_OPEN) continue;
      this.send(client.socket, { type: "event", event });
      delivered++;
    }
    return delivered;
  }

  private matches(client: ClientState, event: ContractEvent): boolean {
    if (
      client.contractIds &&
      client.contractIds.length > 0 &&
      !client.contractIds.includes(event.contractId)
    ) {
      return false;
    }
    if (client.topics && client.topics.length > 0) {
      return event.topics.some((topic) =>
        client.topics!.some((prefix) => topic.startsWith(prefix)),
      );
    }
    return true;
  }

  private handleMessage(clientId: string, data: unknown): void {
    const client = this.clients.get(clientId);
    if (!client) return;

    let message: ClientMessage;
    try {
      message = JSON.parse(String(data));
    } catch {
      this.send(client.socket, {
        type: "error",
        message: "Malformed JSON message",
      });
      return;
    }

    switch (message.type) {
      case "subscribe": {
        const topics = Array.isArray(message.topics)
          ? message.topics.filter((t): t is string => typeof t === "string")
          : [];
        const contractIds = Array.isArray(message.contractIds)
          ? message.contractIds.filter(
              (c): c is string => typeof c === "string",
            )
          : [];
        client.topics = topics;
        client.contractIds = contractIds;
        this.send(client.socket, { type: "subscribed", topics, contractIds });
        break;
      }
      case "unsubscribe":
        client.topics = null;
        client.contractIds = null;
        this.send(client.socket, { type: "unsubscribed" });
        break;
      case "ping":
        this.send(client.socket, { type: "pong" });
        break;
      default:
        this.send(client.socket, {
          type: "error",
          message: "Unknown message type",
        });
    }
  }

  /** One heartbeat sweep — exposed for tests; normally driven by start(). */
  runHeartbeat(): void {
    for (const [clientId, client] of this.clients) {
      if (!client.alive) {
        client.socket.terminate();
        this.clients.delete(clientId);
        continue;
      }
      client.alive = false;
      try {
        client.socket.ping();
      } catch {
        client.socket.terminate();
        this.clients.delete(clientId);
      }
    }
    this.notifyCount();
  }

  private send(socket: StreamSocket, message: ServerMessage): void {
    if (socket.readyState !== SOCKET_OPEN) return;
    try {
      socket.send(JSON.stringify(message));
    } catch {
      // Socket died between the readyState check and the write; the close
      // handler will reap it.
    }
  }

  private notifyCount(): void {
    if (this.onConnectionCountChange) {
      this.onConnectionCountChange(this.clients.size);
    }
  }
}
