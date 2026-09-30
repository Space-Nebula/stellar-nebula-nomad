# Anti-whale mechanisms

Large holders can farm, trade and rank at a scale that pushes everyone else
out of the economy. `src/economics/anti_whale.rs` bounds how much economic
weight one address can exert per day. Every limit is a number the admin or
the governance DAO can change; none of the callers hard-code a threshold.

## What is enforced

| Mechanism | Where it fires | Default | Error |
|-----------|----------------|---------|-------|
| Diminishing returns on gathering | `resource_minter::harvest_resources` | five bands of 120 000 units/day at 100 / 80 / 60 / 40 / 20 % | none (credit is reduced) |
| Daily scan cap | `scan_nebula` | 200 scans/day | contract error 303 (trap) |
| Daily mint cap | `mint_ship`, `batch_mint_ships`, `mint_resource` | 20 mints/day | 303 (trap) / `MinterError::DailyCapExceeded` |
| Daily trade cap | `swap_exact_input`, `place_limit_order` | 100 trades/day | `AmmError::DailyTradeCapExceeded`, `TradingError::DailyTradeCapExceeded` |
| Progressive trading fee | `swap_exact_input` | 0 / 50 / 150 / 300 bps by daily volume band | none (output reduced, fee stays in the last pool) |
| Guild contribution cap | `alliance_manager::contribute_to_treasury` | 100 000 units/member/day | `AllianceError::ContributionCapExceeded` |
| Tiered leaderboards | `update_leaderboard_score` | casual < 200, dedicated < 2 000, hardcore ≥ 2 000 activity points | none |

Daily windows are UTC days (`timestamp / 86 400`). Counters are keyed by
`(player, day)` so old entries expire on their own.

### Diminishing returns
Gathered units are counted per owner per day. The first 120 000 units credit
at 100 %, the next 120 000 at 80 %, then 60 %, 40 %, and everything past
480 000 at 20 %. The effective total for a harvest is spread across its cells
pro rata.

Why 120 000: at the scan rate limit (10 scans/hour) a 16x16 layout yields
about 2 850 units across all assets, so a four-hour session gathers roughly
114 000 units. Casual, regular and hardcore sessions therefore all stay in
the first band at 100 %; the progression targets in ADR 011 hold unchanged.
Two full hardcore sessions in one day (228 000 units) keep about 84 %, and a
bot running around the clock (684 000 units) keeps under 60 %.

### Daily operation caps
Scans, mints and trades are counted per player per day. A batch that would
cross the cap is refused whole and consumes no allowance. `scan_nebula` and
`mint_ship` return plain values rather than `Result`, so a capped call traps
with the contract error code 303 (`OperationCapExceeded`); clients see the
same code they would see from a `Result`.

### Progressive trading fees
Swap output is reduced by an extra fee that depends on the trader's
cumulative daily volume, marginal across bands:

| Daily volume so far | Extra fee |
|---------------------|-----------|
| 0 – 50 000          | 0 bps     |
| 50 000 – 250 000    | 50 bps    |
| 250 000 – 1 000 000 | 150 bps   |
| above 1 000 000     | 300 bps   |

Only the part of a trade that lands in a band pays that band's rate. The fee
is added back to the last pool on the route, so liquidity providers earn it.

### Guild contribution limit
A member can put at most 100 000 units into a treasury per day, so a single
wallet cannot own the guild's decisions by funding it alone.

### Player tiers and leaderboards
Every scan (1 point), mint (5), trade (2) and 1 000 units gathered (1) add
to a lifetime activity score. Players below 200 points are casual, below
2 000 dedicated, and hardcore above that. `update_leaderboard_score` mirrors
every score onto the board of the player's current tier;
`get_tier_leaderboard(tier, limit)` reads them. Boards keep the best score
per player, sorted, capped at 100 rows.

## Exemptions
`set_anti_whale_exempt(admin, user, true)` bypasses every mechanism for a
system account (a treasury bot, an event distributor). Exempt users still
accrue activity so their tier stays meaningful.

## Changing the limits
Two paths, in order of precedence:

1. **Governance.** `governance::set_game_parameter` with one of the symbols
   `aw_width`, `aw_scans`, `aw_mints`, `aw_trades`, `aw_guild`, `aw_dedic`,
   `aw_hard` overrides the matching field for everyone. A zero or negative
   value is ignored; a threshold pair that ends up out of order falls back
   to the stored config for both thresholds.
2. **Admin.** `init_anti_whale_admin(admin)` once, then
   `set_anti_whale_config(admin, config)`. The config is validated: no zero
   limits, dedicated threshold below hardcore threshold.

`get_anti_whale_config` returns the merged result that is actually in
force.

## Monitoring impact
`get_anti_whale_impact` returns lifetime counters:

- `units_gathered_raw` vs `units_gathered_effective`: how much the
  diminishing-returns curve is actually removing. If effective drops far
  below raw for the whole population, the band width is too small for the
  real session length.
- `progressive_fees`: units collected by the progressive fee.

Refusals are not counted on-chain: a capped call fails its invocation and
Soroban rolls back every write made during it, so a failure counter could
never persist. Count them from the failed transactions instead: every cap
refusal emits a `Whale`/`capped` diagnostic event carrying the player, the
operation kind and the cap in force. A rising count spread across many
distinct players (rather than a few) means a cap is hitting engaged players,
not farms.

Successful paths also emit events under the `Whale` topic (`gather`, `fee`,
`config`) so an indexer can attribute the counters to players and compare
retention cohorts before and after a retune.

## Tests
- Unit tests in `src/economics/anti_whale.rs` cover the arithmetic and drive
  the state transitions with the mock storage and clock from `src/traits`.
- `tests/economy/anti_whale_test.rs` goes through the contract client:
  scan and mint caps, the harvest reduction, the guild cap, and tier boards.
