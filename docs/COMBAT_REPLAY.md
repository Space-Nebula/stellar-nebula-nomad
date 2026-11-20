# Combat Replay and Recording System (Issue #592)

The replay system records deterministic PvP combat state, stores it compactly in
Soroban persistent storage, and provides playback, seeking, export/import and
share-link APIs. The PvP engine (`src/pvp_combat.rs`) stays authoritative:
recording is a side-effect hook that never changes combat results or its public
API.

## Data model

### Header — `CombatReplay`

| Field | Type | Meaning |
| --- | --- | --- |
| `replay_id` | `u64` | Monotonic replay id (independent of `combat_id`). |
| `version` | `u32` | Frame layout version (`REPLAY_FORMAT_VERSION = 1`). |
| `combat_id` | `u64` | Source PvP combat id. |
| `player1`, `player2` | `Address` | Duelists; player indexes in frames refer to these. |
| `started_at`, `ended_at` | `u64` | Ledger timestamps; `ended_at == 0` while active. |
| `frame_count`, `final_turn` | `u32` | Number of frames and highest recorded turn (0 = setup only). |
| `winner` | `Option<Address>` | Final winner once resolved. |
| `status` | `Symbol` | `active`, `finished`, or `cancelled`. |
| `integrity_hash` | `BytesN<32>` | SHA-256 of version, combat id, start time, frame count and all packed frames. |

### Frame — `FrameState` / `ReplayFrameView`

Every frame stores a *complete* post-transition combat state, so playback never
re-runs combat logic and seeking never needs to replay earlier moves:

- `kind`: `start`, `move`, `finish`, `cancel`
- `action`: `start`, `attack`, `defend`, `special`, `heal`, `finish`, `cancel`, `none`
- `actor`: 0 (player1), 1 (player2), or `u8::MAX` (no actor)
- `turn_number`: 0 for setup, then 1-based for each move
- `power`, `player1_hp`, `player2_hp`, `player1_energy`, `player2_energy`
- `winner` (0 none, 1 player1, 2 player2), `status`, `timestamp`

`ReplayFrameView` is the decoded, contract-visible form. `FrameState` is a plain
`Copy` struct used by the `Env`-free codec (`pack_frame` / `unpack_frame`).

### Packed frame layout (28 bytes, big-endian)

```
offset  size  field
0       1     kind
1       1     actor
2       1     action code
3       1     status code
4       1     winner code
5       1     turn index (0/1)
6       2     reserved (zero; forward compatibility)
8       2     turn_number
10      2     power
12      2     player1_hp
14      2     player2_hp
16      2     player1_energy
18      2     player2_energy
20      8     timestamp
```

Health, energy and power are bounded by the PvP engine (`MAX_MOVE_POWER = 100`,
`MAX_HP = 100`) and narrow from `u32` to `u16` with saturation. Verified import
rejects any frame whose codes or power fall outside those bounds.

## Recording hooks

Three infallible hooks in `src/combat_replay.rs` are called from
`src/pvp_combat.rs`:

1. `record_start(env, &combat)` — after a new combat is persisted in
   `start_combat`; writes the setup frame and is idempotent per `combat_id`.
2. `record_move(env, &combat, &move_type, power)` — after `execute_move` has
   fully resolved and written the final state; writes a `move` or `finish`
   frame. The actor is derived correctly even though the engine switches
   `combat.turn` for non-finishing moves.
3. `record_cancellation(env, &combat)` — when a combat times out.

Hooks cannot fail a PvP transaction: storage errors, full frame budgets or
already-recorded combats are absorbed. Combats that predate the feature are
lazily seeded from the first observed state.

## Storage, compaction and bounds

Persistent keys (`ReplayKey`):

| Key | Value | Purpose |
| --- | --- | --- |
| `ReplayCounter` | `u64` | Next replay id. |
| `ByCombat(u64)` | `u64` | Combat id → replay id. |
| `Header(u64)` | `CombatReplay` | Metadata and integrity hash. |
| `Chunk(u64, u32)` | `Bytes` | Up to `FRAMES_PER_CHUNK = 32` packed frames (896 bytes). |
| `Share(Symbol)` | `u64` | Share token → replay id. |
| `OwnerReplays(Address)` | `Vec<u64>` | Bounded per-player index (most recent 50). |

All replay entries get `extend_ttl(REPLAY_TTL, REPLAY_MAX_TTL)` (~30 day ledger
sequence window). Storage is space-efficient for three reasons:

- only the initial state plus fixed-width deltas-as-snapshots are stored (no
  `Vec<Symbol>` history or `Address` repeated per frame);
- frames are packed into binary `Bytes` chunks rather than one ledger entry
  each, keeping ~32 frames in a single ~0.9 KB value;
- recording is bounded to `MAX_REPLAY_FRAMES = 512` (16 chunks, ≤ 14,336 bytes
  of frame data per replay) and owner indexes to 50 ids.

Seeking is O(1) for frame index (`chunk = index / 32`, byte offset
`= (index % 32) * 28`) and O(frames) for a turn-number search.

## Serialization / deserialization

- `serialize_replay(env, replay_id) -> ReplayExport` returns the typed header,
  a `format` symbol (`snreplay1`) and a `Bytes` payload concatenating every
  packed frame in order.
- `deserialize_replay(env, caller, export) -> replay_id` requires caller auth,
  checks format/version, exact payload length (`frame_count * 28`), every frame
  code and power bound, and the SHA-256 integrity hash. Only then does it
  allocate a new replay id, rebuild chunks, index both players, and publish a
  `replay/import` event. Existing `ByCombat` mappings are never overwritten.

The integrity hash covers frame data and timing scalars. Addresses live in the
typed, access-controlled header and are not part of the hash.

## Playback and seeking

Playback is frame-driven and deterministic: each tick advances one frame, and
speed changes only the delay between ticks.

| Speed | `speed_bps` | `interval_ms` |
| --- | --- | --- |
| 0.5x | 5,000 | 2,000 |
| 1x | 10,000 | 1,000 |
| 2x | 20,000 | 500 |

`playback_state(env, replay_id, frame_index, speed_bps)` clamps the frame index,
normalizes speed to one of the three supported values, and returns the current
turn, interval, frame count and `at_end` flag.
`seek_to_turn(env, replay_id, turn_number, speed_bps)` scans the packed payload
for the requested 1-based turn (0 = setup frame) and returns the same cursor.
Pure helpers `clamp_frame_index`, `normalize_speed_bps`, `frame_interval_ms`,
`chunk_index`, `frame_offset_in_chunk` and `find_turn_in_packed` are exported for
off-chain use and unit tests.

Events: `replay/start`, `replay/frame`, `replay/final`, `replay/share`,
`replay/import`.

## Sharing link format

Only `player1` or `player2` may call `share_replay(env, caller, replay_id)`.
It returns a stable `Symbol` token:

```
rp<13 base32 chars><1 checksum char>   // exactly 16 chars, URL-safe
```

The frontend builds the canonical link:

```
<origin>/replays?share=<token>
```

`frontend/src/components/ReplayViewer.tsx` exports
`buildReplayShareUrl(token, baseUrl?)`, `parseReplayShareToken(search)`,
`clampFrameIndex` and `intervalForSpeed`. Pass the token from
`get_combat_replay_by_share` (or use the wrapper methods below) to the viewer
as `shareToken`; it calls `onLoadShare(token)` so the parent can fetch the
bundle. The viewer provides play/pause, 0.5x/1x/2x buttons, a frame slider,
previous/next controls and a clickable turn list.

## Contract API

All wrappers live on `NebulaNomadContract` in `src/lib.rs`:

| Method | Returns |
| --- | --- |
| `get_combat_replay_id(combat_id)` | `u64` replay id |
| `get_combat_replay(replay_id)` | `CombatReplay` header |
| `get_combat_replay_frame(replay_id, index)` | `ReplayFrameView` |
| `get_combat_replay_frames(replay_id, start, limit)` | `Vec<ReplayFrameView>` (page ≤ 64) |
| `get_combat_replay_bundle(replay_id)` | `ReplayBundle` (header + frames) |
| `get_player_combat_replays(player)` | `Vec<u64>` |
| `export_combat_replay(replay_id)` | `ReplayExport` |
| `import_combat_replay(caller, export)` | `u64` new replay id |
| `share_combat_replay(caller, replay_id)` | `ReplayShare { token, replay_id }` |
| `get_combat_replay_id_by_share(token)` | `u64` replay id |
| `get_combat_replay_by_share(token)` | `ReplayBundle` |
| `get_replay_playback_state(replay_id, frame_index, speed_bps)` | `PlaybackState` |
| `seek_combat_replay(replay_id, turn_number, speed_bps)` | `PlaybackState` |

Errors (`ReplayError`): `ReplayNotFound`, `InvalidFrame`, `FrameLimitExceeded`,
`CorruptedReplay`, `Unauthorized`, `ShareNotFound`, `UnsupportedFormat`.

## Example flow

```text
create_challenge → accept_challenge (start_combat records setup frame)
  → execute_combat_move × N (one frame each; final move records finish frame)
  → get_combat_replay_id(combat_id)
  → share_combat_replay(player, replay_id) → token
  → link: /replays?share=<token>
  → viewer: parseReplayShareToken → get_combat_replay_by_share → ReplayViewer
```

Tests for frame packing, chunk/seek indexing, speed math, turn scanning and
share-token stability live in the `#[cfg(test)]` module of
`src/combat_replay.rs` and are pure logic (no ledger fixture required).
