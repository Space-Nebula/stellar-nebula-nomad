# Treasure Vault and Loot System

Issue #655. Source: `src/treasure_vault.rs`. UI: `frontend/src/views/TreasureVaultPanel.tsx`.

Treasure vaults lock resources for a fixed period and pay them back with a
10% bonus. The loot system adds an extra reward: once a vault unlocks, it can
be **discovered** to reveal rare loot, which is claimed with a **vault key**.

## Life cycle

```
deposit_treasure ──(lock: 7 days)──► unlocked
                                        │
           ┌────────────────────────────┴───────────────────────────┐
           ▼                                                        ▼
   claim_treasure                                           discover_vault
   principal + 10% bonus                                    rolls loot once on LOOT_TABLE
   forges 1 vault key  ─────────────── key ───────────────► claim_vault_loot
                                                            spends 1 key, pays loot
                                                            (Legendary refunds it + 1 bonus key)
```

Discovery and the principal claim are independent and can happen in either
order. You need a key to claim loot, and you earn keys by completing a lock,
so the loot system rewards locking resources.

## Loot table

| Rarity | Odds | Loot (× vault amount) | Special reward |
| --- | --- | --- | --- |
| Common | 60% | 0.2× | none |
| Rare | 28% | 0.5× | none |
| Epic | 10% | 1.2× | Star Chart |
| Legendary | 2% | 3.0× | Ancient Relic, plus the spent key back and 1 bonus key |

The table lives in `LOOT_TABLE`. Its weights must add up to
`LOOT_ROLL_SPACE` (10,000); a test enforces this.

## Fair distribution

* **Uniform roll:** the roll is `sha256(seed ‖ vault_id ‖ owner)` taken as a
  `u32` modulo 10,000. Every value in `0..10,000` maps to exactly one tier, so
  the odds equal the weights. A test checks all 10,000 rolls. Using a `u32`
  makes the modulo bias smaller than 3·10⁻⁶. The previous draft mapped one
  byte with `byte * 100 / 255`, which skewed the odds.
* **Unique seed:** `randomness_oracle::request_random_seed` mixes the ledger
  sequence, timestamp, network ID and a rolling entropy pool. Mixing in the
  vault ID and owner gives each vault in the same ledger its own roll.
* **Pity timer:** after `PITY_THRESHOLD` (10) consecutive discoveries below
  Epic, the next discovery is at least Epic. The counter resets on any Epic
  or Legendary. `get_pity_counter(owner)` exposes it.
* **One roll per vault:** discovery is stored, and a second attempt fails with
  `AlreadyDiscovered`, so loot can't be rerolled.
* **Known limitation:** on-chain randomness derived from ledger data is
  predictable to a validator, and partly to a player who chooses when to call
  `discover_vault`. The pity timer limits the downside. A commit–reveal or VRF
  source would close the gap and can replace `roll_loot` without changing the
  API.

## Key system

* `claim_treasure` adds 1 key to the owner's balance (`KeyBalance(owner)`).
* `claim_vault_loot` needs at least one key (`KeyRequired`) and spends it.
* Legendary loot refunds the spent key and adds `LEGENDARY_BONUS_KEYS` (1).
* Read the balance with `get_vault_key_balance(owner)`.

## Storage and compatibility

Discovery data lives under new keys (`Discovery(vault_id)`,
`KeyBalance(owner)`, `Pity(owner)`). `TreasureVault` is unchanged, so vaults
already in storage still decode. All loot entry points run under the global
reentrancy guard and follow checks → effects → events.

## Contract API

| Method | Notes |
| --- | --- |
| `deposit_treasure(owner, ship_id, amount)` | unchanged |
| `claim_treasure(owner, vault_id)` | unchanged payout; now also forges a key |
| `discover_vault(owner, vault_id)` | owner only, after the lock; returns `VaultDiscovery` |
| `claim_vault_loot(owner, vault_id)` | owner only; returns `LootReward` |
| `get_vault_discovery(vault_id)` | `Option<VaultDiscovery>` |
| `get_vault_key_balance(owner)` | `u32` |

Events: `vault/discover` (id, rarity, amount), `vault/loot` (id, rarity,
amount), `vault/key` (owner, count).

New errors: `NotDiscovered` (8), `KeyRequired` (9), `LootAlreadyClaimed` (10),
`AlreadyDiscovered` (11).

## Vault UI

`TreasureVaultPanel` shows the key balance as a polite live region, so
screen readers announce changes, and lists each vault as a card:

* **Locked:** the unlock time, formatted for the user's locale.
* **Unlocked:** a "Discover vault" button.
* **Discovered:** the rarity (with a border glow that grows with rarity),
  the loot amount, any special reward, and "Claim loot". With no keys, the
  button stays focusable but `aria-disabled`, and its description explains
  that a key is needed.
* **Claimed:** a "Loot claimed" confirmation.

`pendingVaultId` marks a card as busy while its transaction is in flight, and
repeated clicks are ignored.

## Tests

`cargo test treasure_vault` covers table weights, the exact distribution, tier
ordering and specials, the pity timer, checked loot maths, locked discovery,
the full discover → key → claim flow (including no reroll and no double
claim), ownership checks, claiming before discovery, and the reentrancy guard.
The existing arithmetic and reentrancy tests were commented out upstream
because they called `require_auth` twice in one frame. They now use separate
frames and run again.
