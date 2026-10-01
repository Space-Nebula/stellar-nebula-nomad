# ADR 011: Ship Progression Balance

## Status
Accepted

## Context
Ship upgrades are the main long-term goal in Nebula Nomad. Before this change the only cost model was the module curve in `ship_upgrade.rs`: five slots priced at `base * 1.6^installed`, so a hull with 100-unit blueprints costs 100, 160, 256, 409 and 654 units, 1 579 in total. That schedule is a resource sink, not a progression ladder. A player who harvests at the scan rate limit fills every slot in well under an hour, after which nothing on the ship gets better. There was no notion of level, no way to price a twentieth step, and no analysis of how long each kind of player would take to get there.

The balance targets for the new ladder, from the retention model:

| Play style | Hours per day | Level 5 | Level 10 | Level 20 |
|------------|---------------|---------|----------|----------|
| Casual     | 1             | 1 week  | 1 month  | months   |
| Regular    | 2             | 3 days  | 2 weeks  | months   |
| Hardcore   | 4             | 1 day   | 1 week   | at least 2 months |

## Resource acquisition model
Income is anchored to the live game constants rather than to a guess.

- `rate_limiter::default_nebula_scan` allows 10 scans an hour.
- A 16x16 layout has 256 cells. 15 % are asteroids (ore) with energy `5 + 0..9`, so one harvest yields `256 * 0.15 * 9.5 = 365` ore on average. Every common asset (ore, gas, dust) sits in the same range.
- An active hour is therefore worth 3 650 units of any one common asset. Casual, regular and hardcore players earn 3 650, 7 300 and 14 600 units a day.

Level costs are paid in one asset of the player's choice, so these per-asset rates are the right denominator. The anti-whale diminishing-returns bands only start biting after 60 000 units gathered in a day across all assets, which is above a four-hour session's total, so they do not move these numbers for the three modelled styles.

## Curves considered
All three shapes share a base cost `b` for level 1 and a growth parameter `g`. The simulator in `economics::progression_model` prices each curve, rounds it, sums the cumulative cost and divides by daily income.

| Curve       | Level cost                      | Hardcore, level 5 | Hardcore, level 10 | Hardcore, level 20 |
|-------------|---------------------------------|-------------------|--------------------|--------------------|
| Linear      | `b * (1 + g * (L - 1))`         | 1 day             | 3 days             | 9 days             |
| Logarithmic | `b * (1 + g * log2 L)`          | 1 day             | 2 days             | 4 days             |
| Exponential | `b * (1 + g) ^ (L - 1)`         | 1 day             | 6 days             | 115 days           |

Linear and logarithmic curves cannot satisfy both ends of the table. Scaling their base up until the max level takes two months pushes level 5 out to nine days or more for a hardcore player, which breaks the "first day" hook. The shape is the problem, not the scale: early levels must be cheap and late levels must be an order of magnitude dearer, and only the exponential curve has that ratio.

## Decision
The level ladder uses the exponential curve with `b = 1 449` and `g = 35 %`, applied to levels 1 to 20:

```
raw(L)   = 1449 * 1.35 ^ (L - 1)      (integer arithmetic, floored per step)
price(L) = nearest value to raw(L) ending in 49 or 99
```

The resulting ladder, in units of the chosen asset:

| Level | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|-------|---|---|---|---|---|---|---|---|---|----|
| Cost | 1 449 | 1 949 | 2 649 | 3 549 | 4 799 | 6 499 | 8 749 | 11 849 | 15 999 | 21 549 |

| Level | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 |
|-------|----|----|----|----|----|----|----|----|----|----|
| Cost | 29 099 | 39 299 | 53 049 | 71 649 | 96 699 | 130 549 | 176 249 | 237 899 | 321 199 | 433 599 |

Cumulative: 14 395 to level 5, 79 040 to level 10, 1 668 330 to level 20.

Simulated time to each milestone:

| Play style | Level 5 | Level 10 | Level 20 |
|------------|---------|----------|----------|
| Casual     | 4 days  | 22 days  | 458 days |
| Regular    | 2 days  | 11 days  | 229 days |
| Hardcore   | 1 day   | 6 days   | 115 days |

Every target row is met. `progression_model::meets_retention_targets` encodes the table and the unit tests fail if a constant drifts.

### Psychological pricing
Prices are snapped to the nearest value ending in 49 or 99, ties rounding down, so a price is never more than 25 units off the raw curve. Values below 49 are left alone: rounding 30 up to 49 is a 63 % markup, not a nudge. The snapping keeps the ladder legible in the UI (1 949 reads as "under two thousand") without changing the shape of the curve.

### On-chain implementation
- `ship_upgrade::upgrade_ship_level` burns `price(level + 1)` of the chosen asset from the caller's harvested balance, stores the level under `UpgradeDataKey::ShipLevel`, and tracks the sink under `TotalLevelSpend`.
- `ship_upgrade::level_upgrade_cost` and `level_cost_at` are pure quotes for frontends.
- `ship_upgrade::set_progression_curve` lets the upgrade admin swap in a different base, growth or shape without redeploying; `simulate_progression` reports the days-to-level table for whatever curve is active so a retune can be checked on-chain before it ships.
- The module slot curve from Issue #454 is unchanged. Slots remain a horizontal sink; levels are the vertical ladder.

## Alternatives Considered
- **Time-gated levels (cooldowns instead of cost).** Rejected: cooldowns remove the resource sink and make play intensity irrelevant, which flattens exactly the casual/hardcore spread the targets ask for.
- **Per-asset cost mix per level.** Rejected for now: it multiplies the balance surface by the number of assets and makes the quote harder to display. The single-asset ladder can be extended later without changing storage.
- **Linear curve with a steep base.** Rejected as shown above; it fails the day-one hook.

## Consequences
- **Positive:** progression has a defined end state and a defined pace for three player profiles; the pace is testable and retunable on-chain.
- **Positive:** level 20 costs 1.67 million units of a single asset, a durable sink that scales with the size of the economy.
- **Negative:** the model assumes players harvest at the rate limit and spend on one asset. Real income will be lower for players who split across assets or play in short bursts; the simulator should be re-run against testnet telemetry and the curve retuned through `set_progression_curve` rather than by editing constants.
- **Negative:** balances are `u32`, so a curve priced above 4.29 billion units per level is unaffordable by construction. The validator rejects growth above 1 000 % per level, which keeps the default ladder far inside that bound.
