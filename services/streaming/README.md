# WebSocket Event Streaming Service

Live delivery of Stellar Nebula contract events over WebSockets. Where the
webhook service (`services/webhook`) provides durable at-least-once
delivery to registered HTTP endpoints, this service provides low-latency
fan-out for live UX: leaderboards ticking, scan results appearing, battle
updates streaming into open game clients.

## How it works

```
Soroban RPC ── getEvents poll (cursor + exponential backoff) ──▶ EventStreamer
                                                                    │
                                                              broadcast
                                                                    │
Clients ◀── ws://host:3002/stream ── ConnectionManager (subscriptions, heartbeats)
```

- **EventStreamer** polls Soroban RPC `getEvents` with cursor-based
  resumption, so no events are skipped or duplicated across polls. On RPC
  failure it reconnects with exponential backoff (1s doubling to 60s by
  default) and resumes from the last cursor once the endpoint recovers.
- **ConnectionManager** tracks client sockets: connection cap (refused with
  close code `1013` — try again later), liveness heartbeats that terminate
  sockets missing a pong, and per-client subscription filters applied on
  broadcast.

## Running

```bash
npm install
npm run dev        # ts-node
npm run build && npm start
npm test
```

Configuration (environment variables):

| Variable | Default | Purpose |
| --- | --- | --- |
| `PORT` | `3002` | HTTP + WebSocket listen port |
| `STELLAR_RPC_URL` | `https://soroban-testnet.stellar.org` | Soroban RPC endpoint |
| `CONTRACT_ID` | — | Contract to stream events for |
| `POLL_INTERVAL_MS` | `2000` | RPC poll interval while healthy |
| `HEARTBEAT_INTERVAL_MS` | `30000` | WebSocket liveness ping interval |
| `MAX_CONNECTIONS` | `10000` | Concurrent client cap |

`GET /health` reports stream status, connection count, events streamed,
and the last RPC cursor/error.

## Client protocol

Connect to `ws://host:3002/stream`. All messages are JSON.

Server → client on connect:

```json
{ "type": "connected", "clientId": "client-1" }
```

Subscribe (empty arrays mean "everything"; `topics` are prefix matches):

```json
{ "type": "subscribe", "topics": ["nebula_"], "contractIds": [] }
```

The server acks with `subscribed`, then pushes matching events:

```json
{ "type": "event", "event": { "id": "0007-0001", "contractId": "C…",
  "topics": ["nebula_scanned", "G…"], "value": { }, "ledger": 7,
  "ledgerClosedAt": "2026-01-01T00:00:00Z" } }
```

Also supported: `{ "type": "unsubscribe" }` and `{ "type": "ping" }` →
`{ "type": "pong" }`.

## Client reconnection

The stream is live-only: the server replays nothing on reconnect (durable
delivery is the webhook service's job). Clients should:

1. Reconnect with exponential backoff + jitter (e.g. 1s → 2s → 4s … cap
   30s), resetting the delay after a successful `connected` message.
2. Back off at least the cap before retrying when closed with code `1013`
   (connection limit).
3. Re-send their `subscribe` message after every reconnect — subscriptions
   are per-connection.
4. Treat the `event.id` field as monotonic per contract: if a gap matters
   (e.g. trade history), reconcile via RPC/webhook data rather than
   assuming the stream was complete during the disconnect.
