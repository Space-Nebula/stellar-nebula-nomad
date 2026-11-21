# Guild economy, bounties, yield risk and fractional trading

Contract-side additions for issues #650, #648, #652 and #653. All functions are
exported from the crate root (`src/lib.rs`).

## Guild tax and tribute (`src/guild_economy.rs`)

- `set_guild_tax_rate(founder, alliance_id, tax_bps)`: the alliance founder sets a
  tax of up to `MAX_GUILD_TAX_BPS` (25%). Default is 0.
- `pay_guild_tribute(member, alliance_id, gross)`: splits `gross` into tax and net
  with `split_guild_tribute`. The tax is credited to the alliance treasury and the
  call returns a `TributeReceipt` (`gross`, `tax`, `net`, `tax_bps`, `treasury_after`).
- Treasury spending stays behind member voting: `create_treasury_proposal`,
  `vote_on_proposal`, `execute_proposal` and `get_proposal` are now exported.

## Bounty escrow refunds (`src/bounty_board.rs`)

`cancel_bounty(poster, bounty_id)` returns the escrowed reward to the poster and
removes the bounty. It is allowed while no proof has been submitted, or after the
bounty expired without approval. Approved or disputed bounties cannot be cancelled.
`approve_bounty` and `dispute_bounty` are now exported too.

## Yield risk analysis (`src/yield_forecast.rs`)

`analyze_yield_risk(player)` reads the player's yield history and returns a
`YieldRiskReport`:

| Field | Meaning |
|---|---|
| `average_yield` | Moving average of recorded yields |
| `trend_per_day` | Average change per day |
| `volatility_pct` | Mean absolute deviation as % of the average |
| `level` | `Unknown` (< 2 points), `Low` (≤ 15%), `Medium` (≤ 40%), `High` |
| `recommendation` | `observe`, `hold`, `diversify`, or `reduce` (high risk and falling) |

## Fractional micro-trading (`src/fractional_resources.rs`)

- `list_share_for_sale(seller, share_id, price)` / `cancel_share_listing`.
- `buy_listed_share(buyer, share_id)` moves the share to the buyer, closes the
  listing and records the price per unit for the resource type.
- `get_last_share_price(resource_type)` exposes that last unit price, so prices
  are discovered from real trades. Transferring a share drops its listing.
- Payment for the price is settled by the trading layer; this module records
  ownership and price.
