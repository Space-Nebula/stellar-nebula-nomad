# Quantum Entanglement Communication System (#638)

## Overview
`src/entanglement_comms.rs` provides authenticated, encrypted, rate-limited inter-player and guild-wide messaging across regions on Soroban.

## Contract Entrypoints
- `create_entanglement_pair(owner_a, ship_a, owner_b, ship_b)` — Creates a 30-day pair between two distinct ships.
- `set_pair_use_limit(caller, pair_id, limit)` / `get_pair_remaining_uses(pair_id)` — Enforces per-pair message quotas (default `DEFAULT_PAIR_USE_LIMIT = 100`).
- `set_pair_encryption_key(caller, pair_id, key_hash)` / `get_pair_encryption_key(pair_id)` — Stores a 32-byte key commitment hash for E2E encrypted payloads.
- `send_entangled_message(caller, pair_id, message)` / `send_entangled_message_batch(caller, pair_id, messages)` / `get_entangled_messages(pair_id, offset, limit)` — Sends and paginates encrypted 64-byte messages.
- `create_guild_channel(caller, guild_id, encrypted_topic)` / `send_guild_message(caller, guild_id, sender_region, payload)` / `get_guild_messages(guild_id, offset, limit)` — Integrates with `alliance_manager` for cross-region guild coordination.

## Frontend
- `frontend/src/components/CommsPanel.tsx` renders pair status, remaining uses, message history, and guild broadcast controls.
