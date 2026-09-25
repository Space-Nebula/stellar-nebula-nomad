# Examples

Working examples for each major contract module, in **Rust** (contract
tests, integrations and other Soroban contracts) and **TypeScript** (dApp
frontends and backends).

Every example has step-by-step comments. Error codes used here are listed in
[docs/ERROR_CODES.md](../docs/ERROR_CODES.md).

## Rust

Rust examples run against an in-memory Soroban environment. No network or
deployment is needed.

```bash
cargo run --example <name>
```

| Example | Module(s) | What it shows |
|---------|-----------|---------------|
| [`nebula_exploration`](nebula_exploration.rs) | `nebula_explorer` | Generate a 16×16 nebula, score rarity, one-shot `scan_nebula`, determinism |
| [`nebula_generator`](nebula_generator.rs) | `nebula_gen` | Admin init, validated generation, anomaly queries, TTL expiry, admin sweeps |
| [`ship_nft`](ship_nft.rs) | `ship_nft` | Mint / batch-mint, metadata URIs, owner lookup, transfers, `ShipError` handling |
| [`player_progression`](player_progression.rs) | `player_profile`, `onboarding_tutorial`, `session_manager` | New-player flow: profile → tutorial rewards → ship → session → progress |
| [`pvp_combat`](pvp_combat.rs) | `pvp_combat` | Challenge, accept, turn-based moves, ELO and combat history |
| [`access_control`](access_control.rs) | `access_control` | RBAC bootstrap, roles with expiry, permissions, revoke, admin transfer |
| [`gas_sponsorship`](gas_sponsorship.rs) | `gas_sponsor` | Funding the pool, caps, sponsoring a first scan, one-time enforcement |
| [`storage_optimization`](storage_optimization.rs) | `storage_optim` | TTL-bumped writes, batch reads/writes, composite keys, `CachedEntry` (with CPU costs) |
| [`error_handling`](error_handling.rs) | `nebula_gen`, `ship_nft` | `try_*` results, per-module codes, recover-and-retry policy |
| [`event_scheduler_example`](event_scheduler_example.rs) | `event_scheduler` | Scheduling and running timed community events |
| [`privacy_stats_example`](privacy_stats_example.rs) | `privacy_stats` | Privacy-preserving statistics |
| [`scan_example`](scan_example.rs) | `nebula_explorer` | Walkthrough of the scan algorithm and CLI invocation |

### Using a module from your own contract

Helpers such as `storage_optim` and `gas_sponsor` are plain functions that
take `&Env`. Call them inside your `#[contractimpl]` functions. In tests and
examples, wrap them in `env.as_contract(&contract_id, || { ... })` so they
have a contract storage context.

## TypeScript (dApp integration)

The [`js/`](js/) folder calls a **deployed** contract with
[`@stellar/stellar-sdk`](https://github.com/stellar/js-stellar-sdk).

```bash
cd examples/js
npm install
export NEBULA_CONTRACT_ID=C...            # main NebulaNomadContract
export NEBULA_GEN_CONTRACT_ID=C...        # NebulaGen contract (optional)
export STELLAR_NETWORK=testnet            # testnet | futurenet | local
# export PLAYER_SECRET=S...               # omit to use a Friendbot-funded throwaway account
npm run nebula
```

| Script | npm run | Module(s) | What it shows |
|--------|---------|-----------|---------------|
| [`lib/contract.ts`](js/lib/contract.ts) | — | — | Shared helpers: config, typed ScVal args, `view` (simulate) and `invoke` (simulate → assemble → sign → send → poll), error decoding |
| [`nebula-exploration.ts`](js/nebula-exploration.ts) | `nebula` | `nebula_explorer` | Free preview via simulation, then a real `scan_nebula` |
| [`nebula-generator.ts`](js/nebula-generator.ts) | `nebula-gen` | `nebula_gen` | Get-or-regenerate an expiring layout, query anomalies |
| [`ship-nft.ts`](js/ship-nft.ts) | `ships` | `ship_nft` | Mint, batch mint, metadata, list, transfer, decode `ShipError` |
| [`player-onboarding.ts`](js/player-onboarding.ts) | `onboarding` | `player_profile`, `onboarding_tutorial`, `session_manager` | First-launch flow for a new wallet |
| [`pvp-combat.ts`](js/pvp-combat.ts) | `pvp` | `pvp_combat` | Two signers play a full match |
| [`access-control.ts`](js/access-control.ts) | `rbac` | `access_control` | Admin grants roles; non-admin rejection |
| [`events.ts`](js/events.ts) | `events` | all | Poll `getEvents` for scans, mints and PvP results |
| [`error-handling.ts`](js/error-handling.ts) | `errors` | `nebula_gen`, `ship_nft` | Per-module error tables, category-based retry with backoff |

The higher-level client in [`sdk/`](../sdk/) wraps some of these calls. The
examples use the raw contract interface so every argument type is visible.

### Soroban call flow

```text
view():    build tx ──► simulateTransaction ──► decode retval           (free)
invoke():  build tx ──► simulateTransaction ──► assembleTransaction
                     ──► sign ──► sendTransaction ──► poll getTransaction (fee)
```

Always simulate first: it returns contract errors (`Error(Contract, #N)`)
before the user pays anything.
