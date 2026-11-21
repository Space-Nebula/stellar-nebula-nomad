# Arithmetic Safety Audit

Scope: all arithmetic in `src/`, with emphasis on economy / money modules.
Companion documentation: `docs/YIELD_MATH.md` §7 (patterns), `src/safe_math.rs`.

_Last updated: 2026-11-19 (branch `hardening/ci-dr-arithmetic-reentrancy`)._

## Baseline

| Signal | Count / state |
|---|---|
| `checked_*` calls in `src/` | 145 |
| `saturating_*` calls in `src/` | 346 |
| `wrapping_*` calls in `src/` | 35 — all in PRNG / hash / monotonic-ID code where wrapping is intentional |
| `+ - * / %` / compound assignments | ~500 raw sites, mostly `+= 1` counters |
| `overflow-checks` | `true` in `[profile.release]`; Cargo defaults also enable it in dev/test |
| In-source overflow tests | Present but disabled (`// // #[test]`) in the money modules |

Because overflow traps rather than wraps, an unguarded raw operator is not a
silent-mutation bug — it is a **transaction-abort / DoS** vector: hostile input
can make a public entry point panic. The project's own documented policy
requires a catchable contract error instead.

## Verdict by area

| Area | Verdict |
|---|---|
| Balance modifications (`resource_minter.rs`) | Safe: all credit/debit/move/mint/supply paths already used `checked_*`. One pro-rata scaling expression was raw. |
| AMM / swaps (`trading.rs`) | Mostly safe: swap, LP and fee math already checked. Constant-product helper, integer sqrt and one fee subtraction were raw. |
| Staking / penalties (`staking.rs`) | Mixed: early-withdrawal penalty, unlock-ledger addition and slashing were raw. |
| Revenue split (`content_tools.rs`) | Unsafe: raw basis-point multiplication and a `debug_assert!` invariant that is compiled out of release WASM. |
| Yield forecast (`yield_forecast.rs`) | Unsafe: several raw `i128` multiply/add chains in a read-only calculation. |
| Prize distribution (`prize_distributor.rs`) | Unsafe: raw rank-weight multiplication and unchecked accumulator. |
| `economics/*` ratio helpers | Mixed: several raw divisions with potentially zero denominators. |
| Reward counters (`rewards.rs`, `seasons.rs`, …) | Raw `+=` on bounded counters; low risk but inconsistent with the policy. |

## Fixes applied in this change

| File | Change |
|---|---|
| `src/safe_math.rs` (new) | Shared `checked_mul_div_*`, `saturating_mul_div_i128`, `saturating_add/sub_i128` helpers with 5 unit tests. |
| `src/resource_minter.rs` | Pro-rata harvest scaling now uses `checked_mul_div_u64` → `HarvestError::PriceOverflow`. |
| `src/staking.rs` | Early-withdraw penalty clamped with `saturating_mul_div_i128`; `amount_returned` uses `saturating_sub`; unlock ledger uses `saturating_add`; slashing uses the shared helpers. |
| `src/trading.rs` | `constant_product` uses `saturating_mul`; integer-sqrt steps use `saturating_add`; route fee subtraction uses `saturating_sub_i128`. |
| `src/content_tools.rs` | Revenue split uses `checked_mul_div_i128`; added `ArithmeticOverflow` error; replaced the release-compiled-out `debug_assert!` with a runtime invariant check. |
| `src/prize_distributor.rs` | Rank weights computed with checked/saturating helpers; zero/overflow guards; accumulator uses `saturating_add_i128`. |
| `src/yield_forecast.rs` | Forecast math converted to saturating operations and bounded u32 conversions. |
| `docs/YIELD_MATH.md` | Added §7 documenting when to use `checked_*` vs `saturating_*`, the zero-denominator rule, and test expectations. |

## Verification

```bash
cargo check --lib --locked
cargo test --lib safe_math --locked     # 5 passed
```

Remaining raw-operator sites are concentrated in gameplay counters and the
`economics/*` ratio helpers. They do not move balances directly, but a
follow-up change should convert their divisions to `checked_div` and re-enable
the affected in-source tests. Track that as a separate audit pass rather than
bundling hundreds of mechanical edits into this change.
