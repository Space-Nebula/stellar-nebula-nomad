import express from "express";
import http from "http";
import { WebSocketServer } from "ws";
import dotenv from "dotenv";
import { ConnectionManager } from "./connection-manager";
import { EventStreamer } from "./event-streamer";
import { StreamSocket } from "./types";

dotenv.config();

const PORT = parseInt(process.env.PORT || "3002", 10);
const RPC_URL =
  process.env.STELLAR_RPC_URL || "https://soroban-testnet.stellar.org";
const CONTRACT_ID = process.env.CONTRACT_ID || "";
const HEARTBEAT_INTERVAL_MS = parseInt(
  process.env.HEARTBEAT_INTERVAL_MS || "30000",
  10,
);
const MAX_CONNECTIONS = parseInt(process.env.MAX_CONNECTIONS || "10000", 10);
const POLL_INTERVAL_MS = parseInt(process.env.POLL_INTERVAL_MS || "2000", 10);

const app = express();
app.use(express.json());

const connectionManager = new ConnectionManager({
  heartbeatIntervalMs: HEARTBEAT_INTERVAL_MS,
  maxConnections: MAX_CONNECTIONS,
});

let eventsStreamed = 0;
let lastRpcError: string | null = null;

const streamer = new EventStreamer({
  rpcUrl: RPC_URL,
  contractIds: CONTRACT_ID ? [CONTRACT_ID] : [],
  pollIntervalMs: POLL_INTERVAL_MS,
  onEvent: (event) => {
    eventsStreamed++;
    connectionManager.broadcast(event);
  },
  onError: (error, consecutiveFailures) => {
    lastRpcError = error.message;
    console.error(
      `[streaming] RPC failure #${consecutiveFailures}, retrying in ${Math.round(
        streamer.currentBackoffMs() / 1000,
      )}s:`,
      error.message,
    );
  },
});

/** Liveness/readiness probe with stream health. */
app.get("/health", (_req, res) => {
  res.json({
    status: streamer.isRunning ? "ok" : "starting",
    connections: connectionManager.connectionCount,
    eventsStreamed,
    lastCursor: streamer.lastCursor,
    lastRpcError,
  });
});

const server = http.createServer(app);
const wss = new WebSocketServer({ server, path: "/stream" });

wss.on("connection", (socket) => {
  connectionManager.register(socket as unknown as StreamSocket);
});

if (require.main === module) {
  server.listen(PORT, () => {
    console.log(`[streaming] WebSocket event streaming on :${PORT}/stream`);
    connectionManager.start();
    void streamer.start();
  });

  const shutdown = () => {
    console.log("[streaming] shutting down");
    streamer.stop();
    connectionManager.stop();
    wss.close();
    server.close(() => process.exit(0));
  };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}

export { app, server, connectionManager, streamer };
