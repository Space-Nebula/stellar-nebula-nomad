# Market Oracle and Price Feeds

Issue #654. Source: `src/market_oracle.rs`. UI: `frontend/src/views/MarketOracleDashboard.tsx`.

The market oracle turns signed price observations from independent sources
into one validated, manipulation-resistant price per resource. The rest of
the economy reads that price, for example through the dynamic pricing engine.

## Flow

```
 source A ─┐ submit_source_price
 source B ─┼──────────────────────► SourcePrice(resource, source)   (latest per source)
 source C ─┘
                                          │ aggregate_resource_price (any keeper)
                                          ▼
              fresh? ─► median ─► drop outliers ─► quorum + majority ─► circuit breaker
                                          │
                                          ▼
                     ResourcePrice(resource) + PriceHistory (last 24)
                                          │
              get_current_market_rate ◄───┤ (refuses prices older than 24h)
       dynamic_pricing::observe_oracle_price ◄─┘
```

## Price feeds

* The admin registers sources with `initialize_oracle` (one-shot) and
  `add_oracle_source`, and removes compromised ones with `remove_oracle_source`.
  The list is capped at `MAX_ORACLE_SOURCES` (20) and duplicates are rejected,
  because a source listed twice would get two votes.
* Each registered source calls `submit_source_price(source, resource, price)`
  and must sign the call. Only a source's **latest** observation counts, so
  submitting again replaces the old value instead of adding votes.

## Aggregation and consensus

`aggregate_resource_price(resource)` is permissionless. Its output depends only
on signed observations, so any keeper can run it.

1. **Freshness:** observations older than `MAX_OBSERVATION_AGE_SECS` (1 hour)
   are ignored. Removed sources are ignored immediately.
2. **Median:** take the median of the fresh observations. For an even count,
   use the mean of the middle two, computed without overflow.
3. **Outlier rejection:** observations more than `MAX_SOURCE_DEVIATION_BPS`
   (20%) from that median are discarded.
4. **Consensus:** at least `MIN_ORACLE_SOURCES` (2) observations must remain,
   **and** they must be a strict majority of the fresh observations. Otherwise
   aggregation fails with `InsufficientSources` or `SourceDeviationTooHigh`.
5. The published price is the median of the agreeing observations.
   `PriceData.source_count` records how many sources agreed.

## Validation

* `validate_price` rejects prices that are `<= 0` or above `MAX_PRICE`. It
  applies to source submissions and to admin overrides.
* `batch_update_prices` validates every entry before writing any of them, and
  authenticates the admin once. Calling `update_resource_price` per item would
  call `require_auth` twice in one frame, which the host rejects.
* `get_current_market_rate` returns `StalePrice` for prices older than 24 hours.

## Manipulation resistance

| Attack | Defence |
| --- | --- |
| One or a minority of sources report a fake price | Median plus 20% outlier band; strict-majority consensus |
| A source spams submissions | One observation per source; resubmitting replaces it |
| An unregistered address submits | `SourceNotRegistered`; every submission is signed |
| The same source is registered twice | `DuplicateSource` |
| Someone re-runs `initialize_oracle` to take over the admin role | `AlreadyInitialized` |
| Every source colludes, or a feed bug causes a sudden jump | Circuit breaker: a consensus more than `MAX_PRICE_JUMP_BPS` (50%) from the last fresh price fails with `PriceJumpTooLarge` and emits `oracle/breaker`. The previous price stays in force. |
| Old data is replayed | 1h observation window, 24h published-price window |
| Overflow on extreme values | Saturating deviation maths; overflow-free median |

The admin override (`update_resource_price`) is the recovery path after the
circuit breaker trips. Treat the oracle admin as a high-trust key and keep it
in a multisig.

These guards make manipulation expensive. They do not replace monitoring the
sources: watch `oracle/breaker` and failed aggregations off-chain.

## Integration with pricing

`dynamic_pricing::observe_oracle_price(resource)` (contract method
`observe_oracle_price`) feeds the oracle's fresh consensus price into the EMA
pricing engine. It is permissionless because the input is the validated oracle
price, not something the caller supplies. The print still passes through the
engine's cooldown and deviation guards. If the oracle has no fresh price, it
returns `PricingError::OracleUnavailable`.

## Contract API

| Method | Who | Notes |
| --- | --- | --- |
| `initialize_oracle(admin, sources)` | admin | one-shot |
| `add_oracle_source` / `remove_oracle_source` | admin | |
| `get_oracle_sources()` | anyone | |
| `submit_source_price(source, resource, price)` | registered source | |
| `aggregate_resource_price(resource)` | anyone (keeper) | publishes consensus |
| `update_resource_price` / `batch_update_prices` | admin | validated override |
| `get_current_market_rate`, `get_price_data`, `get_price_history` | anyone | |
| `observe_oracle_price(resource)` | anyone (keeper) | oracle → dynamic pricing |

## Events

`oracle/submit`, `oracle/consensus` (resource, price, agreeing, fresh),
`oracle/price`, `oracle/breaker`, `oracle/source`, `oracle/srcrm`,
`oracle/init`, plus the trigger events `trgger`, `fired` and `disable`.

## Errors

Codes 1–8 are unchanged. New codes: `SourceNotRegistered` (9),
`InsufficientSources` (10), `SourceDeviationTooHigh` (11),
`InvalidTriggerType` (12), `AlreadyInitialized` (13), `DuplicateSource` (14),
`TooManySources` (15), `PriceJumpTooLarge` (16). Codes 10 and 11 are marked
retryable, because more sources may report.

## Fixes to the existing module

* `register_price_trigger` and `disable_trigger` read the admin from
  persistent storage, but `initialize_oracle` writes it to instance storage.
  Both functions therefore always failed with `Unauthorized`.
* `disable_trigger` changed a copy of the trigger and never stored it.
* Trigger IDs came from the ledger sequence, so two triggers created in one
  ledger collided. A counter is now used.
* `initialize_oracle` could be called again by anyone to replace the admin.

## Oracle UI

`MarketOracleDashboard` renders a data table (resource, consensus price,
agreeing sources, age and status) with locale-aware numbers and relative
times. Each status (Validated / Stale / Rejected) is shown as text plus an
icon, never colour alone. A rejected aggregation explains that the last
validated price is still in use.

## Tests

`cargo test market_oracle` covers consensus maths (median, even counts, quorum,
majority, outliers, overflow), validation, end-to-end feeds, stale data,
vote-stacking, the circuit breaker, source removal, one-shot init, duplicates,
admin overrides, atomic batches, bounded history and triggers.
`cargo test dynamic_pricing::tests::oracle` covers the pricing integration.
