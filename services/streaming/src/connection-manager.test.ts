import { ConnectionManager } from "./connection-manager";
import { ContractEvent, SOCKET_OPEN, StreamSocket } from "./types";

type Listener = (...args: any[]) => void;

class FakeSocket implements StreamSocket {
  readyState = SOCKET_OPEN;
  sent: string[] = [];
  pings = 0;
  terminated = false;
  closedWith: { code?: number; reason?: string } | null = null;
  private listeners = new Map<string, Listener[]>();

  send(data: string): void {
    this.sent.push(data);
  }
  ping(): void {
    this.pings++;
  }
  terminate(): void {
    this.terminated = true;
  }
  close(code?: number, reason?: string): void {
    this.closedWith = { code, reason };
  }
  on(event: string, listener: Listener): void {
    const list = this.listeners.get(event) ?? [];
    list.push(listener);
    this.listeners.set(event, list);
  }
  emit(event: string, ...args: unknown[]): void {
    for (const listener of this.listeners.get(event) ?? []) {
      listener(...args);
    }
  }
  lastMessage(): any {
    return JSON.parse(this.sent[this.sent.length - 1]);
  }
}

const EVENT: ContractEvent = {
  id: "0007-0001",
  contractId: "CCONTRACT",
  topics: ["nebula_scanned", "GPLAYER"],
  value: { seed: "42" },
  ledger: 7,
  ledgerClosedAt: "2026-01-01T00:00:00Z",
};

function subscribe(
  socket: FakeSocket,
  filters: { topics?: string[]; contractIds?: string[] } = {},
): void {
  socket.emit("message", JSON.stringify({ type: "subscribe", ...filters }));
}

describe("ConnectionManager", () => {
  it("registers clients and sends a connected handshake", () => {
    const manager = new ConnectionManager();
    const socket = new FakeSocket();
    const clientId = manager.register(socket);

    expect(clientId).not.toBeNull();
    expect(manager.connectionCount).toBe(1);
    expect(socket.lastMessage()).toEqual({ type: "connected", clientId });
  });

  it("refuses connections over the cap with close code 1013", () => {
    const manager = new ConnectionManager({ maxConnections: 1 });
    manager.register(new FakeSocket());
    const refused = new FakeSocket();

    expect(manager.register(refused)).toBeNull();
    expect(refused.closedWith?.code).toBe(1013);
    expect(manager.connectionCount).toBe(1);
  });

  it("does not deliver events to unsubscribed clients", () => {
    const manager = new ConnectionManager();
    const socket = new FakeSocket();
    manager.register(socket);

    expect(manager.broadcast(EVENT)).toBe(0);
  });

  it("delivers events to subscribed clients and acks the subscription", () => {
    const manager = new ConnectionManager();
    const socket = new FakeSocket();
    manager.register(socket);
    subscribe(socket);

    expect(socket.lastMessage()).toEqual({
      type: "subscribed",
      topics: [],
      contractIds: [],
    });
    expect(manager.broadcast(EVENT)).toBe(1);
    expect(socket.lastMessage()).toEqual({ type: "event", event: EVENT });
  });

  it("filters by topic prefix", () => {
    const manager = new ConnectionManager();
    const matching = new FakeSocket();
    const other = new FakeSocket();
    manager.register(matching);
    manager.register(other);
    subscribe(matching, { topics: ["nebula_"] });
    subscribe(other, { topics: ["battle_"] });

    expect(manager.broadcast(EVENT)).toBe(1);
    expect(matching.lastMessage().type).toBe("event");
    expect(other.lastMessage().type).toBe("subscribed");
  });

  it("filters by contract id", () => {
    const manager = new ConnectionManager();
    const socket = new FakeSocket();
    manager.register(socket);
    subscribe(socket, { contractIds: ["COTHER"] });

    expect(manager.broadcast(EVENT)).toBe(0);
  });

  it("stops delivering after unsubscribe", () => {
    const manager = new ConnectionManager();
    const socket = new FakeSocket();
    manager.register(socket);
    subscribe(socket);
    socket.emit("message", JSON.stringify({ type: "unsubscribe" }));

    expect(socket.lastMessage()).toEqual({ type: "unsubscribed" });
    expect(manager.broadcast(EVENT)).toBe(0);
  });

  it("answers application-level pings", () => {
    const manager = new ConnectionManager();
    const socket = new FakeSocket();
    manager.register(socket);
    socket.emit("message", JSON.stringify({ type: "ping" }));

    expect(socket.lastMessage()).toEqual({ type: "pong" });
  });

  it("rejects malformed and unknown messages without dropping the client", () => {
    const manager = new ConnectionManager();
    const socket = new FakeSocket();
    manager.register(socket);

    socket.emit("message", "{not json");
    expect(socket.lastMessage().type).toBe("error");

    socket.emit("message", JSON.stringify({ type: "mystery" }));
    expect(socket.lastMessage().type).toBe("error");

    expect(manager.connectionCount).toBe(1);
  });

  it("removes clients when their socket closes", () => {
    const counts: number[] = [];
    const manager = new ConnectionManager({
      onConnectionCountChange: (count) => counts.push(count),
    });
    const socket = new FakeSocket();
    manager.register(socket);
    socket.emit("close");

    expect(manager.connectionCount).toBe(0);
    expect(counts).toEqual([1, 0]);
  });

  it("skips sockets that are no longer open on broadcast", () => {
    const manager = new ConnectionManager();
    const socket = new FakeSocket();
    manager.register(socket);
    subscribe(socket);
    socket.readyState = 3; // CLOSED

    expect(manager.broadcast(EVENT)).toBe(0);
  });

  describe("heartbeat", () => {
    it("pings live clients and terminates unresponsive ones", () => {
      const manager = new ConnectionManager();
      const responsive = new FakeSocket();
      const dead = new FakeSocket();
      manager.register(responsive);
      manager.register(dead);

      // Sweep 1: both get pinged; only one answers.
      manager.runHeartbeat();
      expect(responsive.pings).toBe(1);
      expect(dead.pings).toBe(1);
      responsive.emit("pong");

      // Sweep 2: the silent socket is reaped.
      manager.runHeartbeat();
      expect(dead.terminated).toBe(true);
      expect(responsive.terminated).toBe(false);
      expect(manager.connectionCount).toBe(1);
    });

    it("runs sweeps on the configured interval once started", () => {
      jest.useFakeTimers();
      try {
        const manager = new ConnectionManager({ heartbeatIntervalMs: 1000 });
        const socket = new FakeSocket();
        manager.register(socket);
        manager.start();

        jest.advanceTimersByTime(1000);
        expect(socket.pings).toBe(1);

        manager.stop();
        jest.advanceTimersByTime(5000);
        expect(socket.pings).toBe(1);
      } finally {
        jest.useRealTimers();
      }
    });
  });

  it("closes all clients on stop", () => {
    const manager = new ConnectionManager();
    const socket = new FakeSocket();
    manager.register(socket);
    manager.stop();

    expect(socket.closedWith?.code).toBe(1001);
    expect(manager.connectionCount).toBe(0);
  });
});
