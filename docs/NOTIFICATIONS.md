# Smart Alerts and Player Notifications

Two layers:

| Layer | Module | Audience |
| --- | --- | --- |
| Operator alerts on metric thresholds | `src/smart_alerts.rs` | Ops / monitoring |
| Player notifications | `src/notifications/push_service.rs`, `src/notifications/alerts.rs` | Players |

Both are module APIs called from other contract code. They are not contract
entry points.

## Operator alerts (`smart_alerts`)

### Thresholds and priorities

```rust
configure_threshold(&env, symbol_short!("errs"), 10);                       // Medium priority
configure_threshold_with_priority(&env, symbol_short!("tvl_drop"), 5, AlertPriority::Critical);
```

`AlertPriority` is `Low < Medium < High < Critical`. Every alert copies the
priority of its threshold.

### Evaluation, escalation and aggregation

`evaluate_sample(&env, metric, value)`:

- `value < max_value`: healthy. The breach streak resets and the call returns `Ok(None)`.
- Otherwise the streak grows: 1st breach `Warning`, 2nd–3rd `Critical`, 4th+ `PageOps`.
- **Aggregation:** if the newest alert for this metric is unacknowledged and was
  last seen at most `AGGREGATION_WINDOW_SECS` (1 hour) ago, that alert is
  updated in place (`occurrences += 1`, new `value`, `severity`, `last_seen`)
  and returned. One ongoing incident stays one alert instead of a flood.
- Otherwise a new `AlertRecord` is appended (log capped at 30, oldest dropped).

`acknowledge(&env, alert_id)` marks the alert handled and resets the streak.
The next breach opens a new alert at `Warning`.

### Reading

- `list_active_alerts(&env)`: the whole log, oldest first.
- `list_open_alerts(&env, min_priority)`: unacknowledged alerts with
  `priority >= min_priority`, highest priority first, newest first within a priority.

`AlertRecord` fields: `alert_id`, `metric`, `value`, `threshold`, `severity`,
`timestamp` (first breach), `acknowledged`, `priority`, `occurrences`, `last_seen`.

## Player notifications (`push_service`)

### Priorities

`NotificationPriority`: `Low < Normal < High < Urgent`.

Built-in senders (`notifications::alerts`):

| Function | Message | Priority |
| --- | --- | --- |
| `check_low_resources(balance, threshold)` | `low_res` | `High`, or `Urgent` when `balance == 0` |
| `notify_rare_discovery` | `rare_find` | `Normal` |
| `notify_crafting_complete` | `craft_ok` | `Low` |
| `emit_notification` (legacy) | any | `Normal` |

### Preferences (customization)

```rust
set_notification_preferences(&env, player.clone(), NotificationPreferences {
    min_priority: NotificationPriority::Normal,
    channels: CHANNEL_IN_APP | CHANNEL_PUSH,
    muted: vec![&env, symbol_short!("craft_ok")],
})?; // player.require_auth()
```

- `min_priority`: anything below it is dropped.
- `channels`: bitmask of `CHANNEL_IN_APP` (1), `CHANNEL_PUSH` (2) and `CHANNEL_EMAIL` (4).
  Other bits are rejected with `NotificationError::InvalidChannels`.
- `muted`: up to `MAX_MUTED` (16) message types that are dropped (`TooManyMuted` otherwise).
- `Urgent` ignores `min_priority` and `muted`, and is always delivered in-app.
- Defaults (no preferences saved): `Low`, in-app only, nothing muted. This is
  exactly the old behaviour.

### Delivery (multi-channel)

`notify(&env, player, message, priority) -> Option<Notification>` stores the
notification in the player's inbox and emits one event per enabled channel:

| Channel | Topics | Data |
| --- | --- | --- |
| In-app | `("notify", player)` | `message` (unchanged legacy shape) |
| Push | `("notify", "push", player)` | `(message, priority)` |
| Email | `("notify", "email", player)` | `(message, priority)` |

The contract cannot send pushes or emails. Off-chain relayers subscribe to the
push and email topics and deliver them.

### Aggregation

If the inbox already has the same `message` from at most
`NOTIFICATION_AGGREGATION_SECS` (1 hour) ago, that entry's `count` goes up,
its `timestamp` moves to now, and its priority becomes the higher of the two.
**No new events are emitted**, so a burst of identical alerts reaches the
player's devices once.

### Inbox

- `get_inbox(&env, &player)`: up to `MAX_INBOX` (20) entries, oldest first.
- `clear_inbox(&env, player)`: player-authorised.
- The inbox lives in temporary storage. If it expires, nothing breaks and the
  next notification starts a fresh inbox. Events remain the durable record.

## Frontend

`NotificationCenter` (in `frontend/src/views/NotificationCenter.tsx`, rules
mirrored in `frontend/src/utils/notifications.ts`) shows the inbox in priority
order. It shows aggregated repeats as `×N`, offers mute and unmute per message,
and edits the minimum priority and channels. Changes come back through
`onPreferencesChange`, and the app submits them with `set_notification_preferences`.

## Complexity

- `evaluate_sample`: O(A) over the alert log (A ≤ 30).
- `list_open_alerts`: O(4·A).
- `notify`: O(I + M) over the inbox (I ≤ 20) and muted list (M ≤ 16).
