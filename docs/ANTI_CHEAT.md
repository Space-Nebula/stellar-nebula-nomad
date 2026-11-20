# Anti-Cheat Detection for PvP Combat (Issue #593)

`src/anti_cheat.rs` adds a server-side defence layer around `src/pvp_combat.rs`.
The contract remains the only authority for combat state: a client submits a
move type and power, and the contract derives HP/energy changes itself. The
anti-cheat module validates inputs, compares the applied state with that
derivation, monitors action timing, maintains a suspicion score, applies
automatic penalties, and exposes admin review + player appeals.

All timestamps are **ledger seconds** (the same unit as
`pvp_combat::COMBAT_TIMEOUT`). No dependencies were added.

## 1. Coverage: every PvP action is validated

| PvP entry point | Anti-cheat hook |
| --- | --- |
| `create_challenge` | `enforce_allowed` (ban gate) + `observe_action` |
| `accept_challenge` | ban gate, clock-rollback detection, `observe_action` |
| `decline_challenge` | ban gate + `observe_action` |
| `execute_move` | ban gate, `observe_action`, `validate_move`, `verify_state_transition` |
| `join_matchmaking` / `leave_matchmaking` | ban gate + `observe_action` |
| `add_spectator` / `remove_spectator` | ban gate (spectate also observed) |

`enforce_allowed` is a no-op for legitimate players, so existing behaviour and
the `PvPError` API are unchanged for honest play. Banned callers receive
`PvPError::Unauthorized` (the existing error variant); no new PvP error codes
were introduced.

Detected exploit attempts on `execute_move` (out-of-range power, out-of-turn
submission, insufficient energy, unknown move type) are rejected with the
existing `PvPError::InvalidMove` before any combat state is written. The
detection path logs the attempt and updates the suspicion profile, but because
Soroban rolls back all storage on `Err`, its persistent record is only kept for
detections raised on successful calls (timing anomalies, impossible state
changes and repeat offenders). Legitimate moves follow the original code path
and the established `PvPError` contract is preserved.

## 2. Detection rules

| `DetectionType` | Trigger | Base score |
| --- | --- | --- |
| `InvalidAction` | Impossible input: bad power, energy charge, move type, or out-of-turn submission | 10 |
| `ImpossibleState` | Post-move HP/energy/turn differs from the server-derived expectation; damage/heal bounds exceeded | 30 |
| `TimingAnomaly` | Two actions closer than `min_action_interval_secs`, or a timestamp moving backwards | 10 |
| `PatternAnomaly` | ≥80% of consecutive gaps within 1s of each other across ≥5 samples | 12 |
| `BurstAnomaly` | More than `max_actions_per_window` actions inside `action_window_secs` | 12 |

`score = min(100, base × severity_multiplier)` with `Low = 1`, `Medium = 2`,
`High = 3`, `Critical = 4`. A single `ImpossibleState`/`Critical` detection
therefore reaches the 100 cap immediately. Detections are rate-limited by a
30-second cooldown so one burst cannot flood the queue, and scores halve once
per week of inactivity (bounded decay).

### Impossible state changes

Before a move is applied, `validate_move` derives a `MoveValidation` containing
the exact expected HP/energy snapshot (`power / 2` energy cost, `power` attack
damage, `2 × power` special damage, `power / 2` heal capped at `MAX_HP`, `+10`
energy on defend). After the engine applies the move, `verify_state_transition`
compares the actual snapshot against that expectation. A mismatch emits a
Critical `ImpossibleState` detection and restores the expected HP/energy values
*before* winner resolution, so impossible damage cannot change the match
result. Turn integrity is also checked (finished combats are exempt).

### Timing manipulation

`observe_action` keeps a bounded 16-entry per-player timestamp window and
checks:

- `now < last` → clock rollback (`TimingAnomaly`, High via challenge acceptance),
- `now - last < min_action_interval_secs` → low-signal timing anomaly,
- burst and regular-interval detection via pure helpers.

## 3. Scoring thresholds and penalties

Defaults (`AntiCheatConfig::default()`):

| Setting | Default |
| --- | --- |
| `warning_threshold` | 40 |
| `temp_ban_threshold` | 65 |
| `permanent_ban_threshold` | 90 |
| `review_threshold` | 50 |
| `min_action_interval_secs` | 2 |
| `action_window_secs` / `max_actions_per_window` | 60 / 8 |
| `detection_cooldown_secs` | 30 |
| `temp_ban_duration_secs` | 86,400 (1 day) |
| `appeal_window_secs` | 604,800 (7 days) |
| `max_detections_per_player` / `max_review_queue` | 50 / 200 |

Penalty escalation is deterministic:

- score ≥ `permanent_ban_threshold` → `PermanentBan`;
- `Critical` severity, score ≥ `temp_ban_threshold`, or 3 prior warnings →
  `TemporaryBan` (expires after `temp_ban_duration_secs`);
- score ≥ `warning_threshold` → `Warning` (increments warning count);
- otherwise no penalty.

A `Warning` or ban is stored as a `PenaltyRecord` and applied automatically in
the same detection transaction. The next PvP action by a banned player fails at
the ban gate. Admins can tune every threshold with `ac_set_config`; invalid
configs (unordered thresholds, zero windows) are rejected with
`InvalidConfig`.

## 4. Detection logging

Every detection writes a persistent `DetectionRecord` (id, player, rule,
severity, score delta/after, optional combat id, action, evidence detail,
timestamp, auto-penalty, review state (`Pending` until resolved)) **and** emits
an event:

- `("ac", "detect")` → `(player, detection_id, rule, severity, score_after)`
- `("ac", "penalty")` → `(player, penalty, detection_id, expires_at)`
- `("ac", "review")` → `(player, detection_id)` when queued for review
- `("ac", "reviewed")` / `("ac", "appealed")` for admin decisions
- `("ac", "appeal")` when a player appeals

Per-player detection ids are stored in a bounded list, and the review queue is
a bounded list of pending ids. `ac_get_detection`, `ac_get_player_detections`
and `ac_get_review_queue` expose them to indexers and the dashboard.

## 5. Manual review and appeals

1. Any `Medium`+ detection, or a score at/above `review_threshold`, enters the
   manual review queue.
2. Admin calls `ac_resolve_review(detection_id, decision, reason)` with
   `Cleared` (restore score, lift this detection's penalty), `Upheld` (keep
   penalty), or `Reversed` (reset score, clear warnings and penalty). `Pending`
   is the stored initial state and is rejected as an admin decision with
   `InvalidDecision`. Resolving removes the id from the queue and is
   idempotence-guarded with `AlreadyResolved`.
3. A penalised player calls `ac_submit_appeal(detection_id, reason)` within
   `appeal_window_secs` of the detection. One appeal per detection is allowed.
4. Admin calls `ac_resolve_appeal(appeal_id, approve, reason)`. Approval
   reverses the underlying detection, resets the profile, clears the penalty
   and removes the item from the review queue; rejection leaves it intact.

Admin actions require the anti-cheat admin established by `ac_set_admin` (first
caller bootstraps; later calls rotate only by the current admin).

## 6. Contract API

| Method | Purpose |
| --- | --- |
| `ac_set_admin(caller, admin)` | Bootstrap/rotate anti-cheat admin |
| `ac_set_config(caller, config)` | Update thresholds (admin) |
| `ac_get_config()` | Effective config |
| `ac_get_profile(player)` | Score, counts, penalty, ban expiry |
| `ac_is_banned(player)` | Active ban check |
| `ac_get_detection(id)` | Single detection record |
| `ac_get_player_detections(player, limit)` | Recent detections |
| `ac_get_review_queue(limit)` | Pending detections |
| `ac_resolve_review(admin, id, decision, reason)` | Admin review decision |
| `ac_submit_appeal(player, detection_id, reason)` | Player appeal |
| `ac_get_appeal(id)` / `ac_resolve_appeal(admin, id, approve, reason)` | Appeal lookup/decision |

Module-level helpers in `anti_cheat.rs` (used by the PvP hooks) are also
re-exported at the crate root: `AntiCheatConfig`, `AntiCheatError`,
`AntiCheatDetection`, `AntiCheatProfile`, `AntiCheatPenalty`,
`AntiCheatDetectionType`, `AntiCheatSeverity`, `AntiCheatReviewDecision`,
`AntiCheatAppeal`, `AntiCheatAppealStatus` and the threshold constants.

`AntiCheatError` has its own `StandardContractError` descriptor namespace
(`module = "anti_cheat"`), so its codes do not collide with `PvPError`.

## 7. Frontend

`frontend/src/components/AntiCheatReviewDashboard.tsx` (exported from the
components barrel) renders the review queue, severity filtering, detection
detail with Clear/Uphold/Reverse actions, the appeals list with
Approve/Reject, and the effective threshold table. It takes data and callbacks
as props, so it can be backed by `ac_get_*` / `ac_resolve_*` calls without new
npm dependencies.

## 8. Tests

`anti_cheat.rs` contains pure-logic unit tests (no Soroban env) covering
exploit attempts and deterministic behaviour: impossible damage/heal/energy,
state mismatch, score capping and decay, penalty escalation, timing rollback,
burst/regular-pattern detection, cooldown and config validation. Run with the
normal `cargo test` suite; none require network or new dev-dependencies.
