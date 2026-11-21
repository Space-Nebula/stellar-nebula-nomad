# Reentrancy Protection

This document records the verification of `src/reentrancy_guard.rs`, the
external-call surface of the contract, and the attack scenarios that now guard
it (Issue #472 / Issue #238).

_Last updated: 2026-11-19 (branch `hardening/ci-dr-arithmetic-reentrancy`)._

## 1. Threat model

Soroban executes cross-contract calls atomically: a callee runs to completion
before the caller resumes. That removes the classic EVM "call back into the
caller before state is written" vector **for outbound calls**.

It does **not** remove the inbound vector. When a contract calls
`address.require_auth()` and `address` is a **custom-account contract**, the
account's `__check_auth` function is arbitrary code executed while the caller's
transaction is still in progress. That function can attempt to re-enter any
entry point of this contract. The guard below is the defense-in-depth mutex for
that case.

Consequently, `docs/SECURITY.md` and `docs/THREAT_MODEL.md` should be read with
this distinction in mind: "Soroban is reentrancy-safe" applies to outbound
calls; the explicit guard covers the `require_auth` callback path.

## 2. The guard

`src/reentrancy_guard.rs` exposes:

| API | Purpose |
|---|---|
| `acquire(&Env)` | Acquire the contract-wide mutex; returns `ReentrancyError::ReentrantCall` if already held. |
| `release(&Env)` | Release the mutex (call only after a successful `acquire`). |
| `is_locked(&Env)` | Inspect the lock flag. |
| `with_guard(&Env, FnOnce() -> Result<T, E>)` | Acquire, run the body, release on both success and error return. |

The flag lives in **instance storage**. A failed Soroban invocation rolls back
all instance-storage writes, so a panic cannot leave the contract permanently
locked. Error returns are handled explicitly by `with_guard`.

`with_guard` requires `E: From<ReentrancyError>`; every module that uses it
implements that conversion into its own domain error (`AmmError::Reentrancy`,
`MinterError::Reentrancy`, `ComposabilityError::Reentrancy`, …).

### Nesting rule

The lock is **global**, not per-function, so it also blocks cross-function
reentrancy. Therefore guarded entry points that compose each other must not
nest `with_guard`. Each public guarded function is a thin wrapper around a
private `*_unguarded` body; composite paths call the unguarded body while
already holding the lock.

## 3. Audit of the external-call surface

| Category | Finding |
|---|---|
| Outbound cross-contract calls (`env.invoke_contract`, `try_invoke`, generated clients in production code) | **None.** `src/` and `examples/` contain no real cross-contract invocations. `composability_examples.rs` documents the intended call site and keeps the guard around it. |
| `require_auth` sites | Hundreds — every state-mutating entry point. This is the actual inbound callback surface. |
| Guarded entry points | 20 production sites across `resource_minter`, `dex_integration`, `trading`, `escrow_trader`, `nomad_bonding`, `treasure_vault`, plus the composability example. |
| Independent local locks | `ship_nft::{mint_ship, transfer, …}` (persistent key) and `storage_optim::{store_with_bump, …}` (instance key). These do **not** share the global lock. |
| Documentation mismatch found | `resource_minter::mint_resource` claimed to run under the guard but did not; its `mint_resource_unguarded` body had no callers. **Fixed** — the public function now delegates to `mint_resource_unguarded` inside `with_guard`. |
| Disabled tests found | All 5 tests in `reentrancy_guard.rs` had `// // #[test]`. **Fixed** — re-enabled; all 5 pass. |

### Guarded entry points

- `resource_minter`: `mint_resource`, `harvest_resources`, `auto_list_on_dex`
- `dex_integration`: `harvest_and_list`, `list_resource`, `buy_offer`, `sell_to_order`, `cancel_listing` (`list_at_market` is protected transitively)
- `trading`: `place_limit_order`, `cancel_limit_order`, `record_trade`, `add_liquidity`, `remove_liquidity`, `swap_exact_input`
- `escrow_trader`: `complete_escrow`, `cancel_escrow`
- `nomad_bonding`: `delegate_yield`, `claim_yield`, `dissolve_bond`
- `treasure_vault`: `deposit_treasure`, `claim_treasure`
- `composability_examples`: `compose_with_external_contract`, `malicious_callback_attempt`, `record_composition_gas`

Low-level ledger primitives (`credit_balance`, `debit_balance`, `move_balance`,
`reduce_supply`) are deliberately unguarded: they are reachable only from
already-guarded or auth-checked paths, and guarding them would make those
callers self-block.

## 4. Test coverage

| Suite | Tests | What it verifies |
|---|---|---|
| `src/reentrancy_guard.rs` (re-enabled) | 5 | Reentrant call blocked, sequential calls release the lock, cross-function reentry blocked, error path releases lock, acquire/release round-trip |
| `src/composability_examples.rs` (new) | 3 | Malicious callback reentry rejected, sequential compositions, telemetry write guarded |
| `src/trading.rs` (existing, active) | 4 | Swap / liquidity / limit-order operations rejected while the lock is held |
| `tests/security/reentrancy_attack_tests.rs` (new) | 6 | End-to-end contract-boundary attack scenarios: reentrant callback, honest call, cross-function reentry, error path, sequential calls, isolation between frames |

Run them with:

```bash
cargo test --lib reentrancy_guard --locked
cargo test --lib composability_examples --locked
cargo test --test reentrancy_attack_tests --locked
cargo test --lib trading::tests --locked
```

## 5. Patterns to follow when adding a guarded function

1. Authenticate the acting address (`require_auth`).
2. Acquire the global lock (usually via `with_guard`).
3. Inside the lock: validate inputs, write all state changes (Checks → Effects),
   then publish events / perform interactions.
4. Do **not** call another `with_guard`-wrapped function from inside the lock —
   call its `*_unguarded` body instead.
5. Add an attack test that holds the lock and asserts the guarded function
   returns the module's reentrancy error.

## 6. Residual risks / follow-ups

- The independent locks in `ship_nft` and `storage_optim` do not exclude the
  global lock. A path that holds one and then calls a globally-guarded function
  will not self-block, which is usually desirable, but auditors should be aware
  that "the contract" has three distinct mutexes.
- Many `require_auth` entry points outside the 20 guarded ones rely on
  checks-effects-interactions ordering rather than an explicit lock. They are
  not proven exploitable, but a future composability change should re-audit them.
- `docs/SECURITY.md` and `docs/THREAT_MODEL.md` still use the shorthand
  "Soroban is inherently reentrancy-safe". See §1 for the required nuance.
