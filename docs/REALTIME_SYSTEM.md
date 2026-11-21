# Real-time update system

Frontend real-time updates are centralized in `frontend/src/services/realtime.ts`.

- `RealtimeClient` wraps `WebSocketClient` and exposes channel subscriptions, status changes, notification handlers, state snapshots, and state deltas.
- `REALTIME_CHANNELS` lists channels used by the UI.
- On every successful connection the client sends a `snapshot` request for subscribed channels, then applies `state_snapshot` and `state_delta` messages to an in-memory state map.
- Backoff, heartbeat, throttling, and resubscription are handled by `WebSocketClient`.
- Incoming messages are normalized in `WebSocketClient`, so backend event envelopes, notifications, batch events, and state messages all reach subscribers as `WebSocketEvent` values.

## Usage

```ts
const realtime = new RealtimeClient();
realtime.subscribe(REALTIME_CHANNELS.LEADERBOARD, (event) => render(event.payload));
realtime.onNotification((notification) => toast(notification.message));
realtime.connect();
```

Call `disconnect()` when the owning component unmounts.
