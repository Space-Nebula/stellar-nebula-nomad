# Yield Farming Math

All yield calculations in `src/yield_farming.rs` use checked integer arithmetic in basis points (`10_000 bps = 100%`) to avoid floating-point nondeterminism on Soroban.

## Staking Tiers

| Tier | Minimum Stake | Reward Multiplier |
|---|---:|---:|
| Bronze | `< 5,000` | `10,000 bps` (`1.00x`) |
| Silver | `5,000` | `11,000 bps` (`1.10x`) |
| Gold | `25,000` | `12,500 bps` (`1.25x`) |
| Platinum | `100,000` | `15,000 bps` (`1.50x`) |

## Time-Weighted Multiplier

`time_weight_bps(elapsed_secs, lock_secs)` scales linearly from `10,000 bps` (`1.00x`) at `elapsed = 0` up to `15,000 bps` (`1.50x`) once `elapsed >= lock_secs`:

```text
weight_bps = 10_000 + floor(min(elapsed_secs, lock_secs) * 5_000 / lock_secs)
```

## Compound APY & Rewards

- **Effective APY (`calculate_apy_bps`)**: computes `((1 + apr / n)^n - 1)` in basis points using `1e9` fixed-point exponentiation by squaring.
- **Compound Reward (`compound_reward`)**: applies `calculate_farm_reward` across `periods` compounding intervals (plus any remainder seconds) with checked addition so overflows return `None`.

## Arithmetic safety patterns

The contract-level policy (see the `[profile.release]` comment in `Cargo.toml`)
is that arithmetic must never silently wrap. Two complementary mechanisms are
used, and choosing the right one is part of the code review checklist for any
change that touches balances, rewards, or supply:

### `checked_*` — fail the transaction with a contract error

Use for **state mutations**: credits, debits, supply changes, fee splits, and
anything that ends up in storage. A failed `checked_*` operation is converted
into the module's domain error (e.g. `MinterError::ArithmeticOverflow`,
`ContentToolsError::ArithmeticOverflow`), which aborts the transaction with a
diagnosable code instead of a raw overflow trap.

```rust
let new_balance = current
    .checked_add(amount)
    .ok_or(MinterError::ArithmeticOverflow)?;
```

### `saturating_*` — clamp derived values

Use for **derived, non-state values** where aborting a read-only call would be
disproportionate: yield forecasts, confidence scores, monotonic counters close
to their type limit, and defensive backstops in helpers such as `constant_product`.

```rust
let predicted_yield = adjusted_daily.saturating_mul(i128::from(days));
```

Never use saturating arithmetic to hide an impossible balance condition — that
would silently create or destroy value.

### Shared helpers (`src/safe_math.rs`)

| Helper | Behaviour |
|---|---|
| `checked_mul_div_i128` / `checked_mul_div_u64` | Multiply before dividing to preserve precision; `None` on overflow or zero denominator. |
| `saturating_mul_div_i128` | Same order of operations, but clamps; for forecast/display math only. |
| `saturating_add_i128` / `saturating_sub_i128` | Clamp at the type bounds. |

Multiply-before-divide is intentional: `(7 × 10) / 3 = 23`, whereas
`(7 / 3) × 10 = 20`. Truncation happens once, at the end.

### Division by zero

Every division whose denominator can originate from configuration or storage
must use `checked_div` (or the helpers above, which reject zero). Raw `/` is
only acceptable for compile-time non-zero constants such as `10_000` (BPS) and
`6_307_200` (`LEDGERS_PER_YEAR`).

### Tests

Overflow scenarios live in `src/safe_math.rs` (unit tests for the helpers) and
in `tests/fuzz/` / `tests/chaos/byzantine_attacks.rs` (extreme-value scenarios).
When fixing a raw-arithmetic site, add a test that drives the actual call with
`i128::MAX`/`u64::MAX` inputs and asserts the domain error rather than a panic.
