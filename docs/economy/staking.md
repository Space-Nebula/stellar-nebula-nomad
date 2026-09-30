# Staking and yield farming

Five ways to lock value, all paying out in harvested resource units and all
counted in the TVL report. The arithmetic lives in
`src/economics/apy_calculator.rs` (pure functions, no storage); the on-chain
products live in `src/yield_farming.rs` (resource, NFT and LP staking) and
`src/staking.rs` (guild and governance staking).

## Resource staking

| Tier     | Lock    | APY  |
|----------|---------|------|
| `Days7`  | 7 days  |  5 % |
| `Days30` | 30 days | 15 % |
| `Days90` | 90 days | 35 % |

- `stake_resource(owner, asset, amount, tier)` debits `amount` from the
  owner's harvested balance and returns a stake id. The APY is fixed at
  deposit.
- Yield is simple interest, pro-rated by the second, and stops accruing at
  the end of the lock: `principal * apy * elapsed / (10 000 * 31 536 000)`.
  10 000 units for 90 days at 35 % earns 863 units.
- `claim_resource_yield(owner, id)` pays what has accrued so far;
  `unstake_resource(owner, id)` returns the principal plus unclaimed yield
  once the lock has passed and fails with `LockNotMet` before that.
- Yield is paid from a per-asset reserve. `fund_yield_reserve(funder, asset,
  amount)` moves units from the funder's balance into it; a claim that the
  reserve cannot cover fails with `YieldReserveEmpty` and can be retried
  once the reserve is topped up. The principal is never at risk: it is held
  separately from the reserve.

### APY dampener
Above a per-asset TVL target (default 10 million units) the quoted APY falls
in proportion, `base * target / tvl`, with a floor of 20 % of base. New
deposits into an over-subscribed pool earn less; existing stakes keep the
rate they locked. `set_tvl_target` (staking admin) moves the knee.

### Emergency withdrawal
`emergency_unstake_resource(owner, id)` before the unlock returns 50 % of the
principal. The other 50 % and all accrued yield go into the asset's yield
reserve, so early leavers fund patient stakers. After the unlock the call is
a normal withdrawal with no penalty.

## NFT (ship) staking
- `stake_ship(owner, ship_id)` freezes the ship: `transfer_ownership` fails
  with `ShipError::Staked` until it is unstaked. The ship stays in the
  owner's wallet, so nothing else about it changes.
- Daily yield is `10 * level * rarity`, where `level` is the ship's
  progression level at deposit (minimum 1) and `rarity` is a hull multiplier:
  fighter 1.0, explorer 1.1, hauler 1.2. A level 5 explorer earns 55 units a
  day. Re-stake after levelling up to pick up the new rate.
- Yield is paid in the asset set by `set_nft_yield_asset` (default `dust`)
  from that asset's reserve. `claim_ship_yield` and `unstake_ship` pay it.

## Liquidity provision staking
- `stake_lp(provider, pool_id, amount)` moves LP units out of the provider's
  AMM balance into a per-pool staking position.
- `fund_lp_rewards(funder, pool_id, amount)` deposits the pool's reward asset
  (resource A of the pool): 80 % is distributed to current stakers pro rata
  through a reward-per-share accumulator, 20 % goes into an impermanent-loss
  pot. With nobody staked, the whole deposit goes to the pot.
- `claim_lp_rewards` pays the pending share. `unstake_lp(provider, pool_id,
  amount)` pays pending rewards, returns the LP units, and compensates
  impermanent loss.

### Impermanent-loss protection
At withdrawal the position's divergence loss is computed from the pool's
price ratio at deposit versus now (`1 - 2*sqrt(r) / (1 + r)`). Protection
vests linearly from 0 % at deposit to 100 % after 90 days. The protected
fraction of the loss, applied to the withdrawn LP units, is paid from the
pool's IL pot, capped by what the pot holds. A 4x price move (20 % loss) on a
position held 45 days is compensated at 10 %.

## Guild staking
- `stake_to_guild(member, asset, amount)` locks units behind the caller's
  alliance for at least seven days. Every stake in a guild uses the asset the
  guild started with. The locked units are credited to the alliance treasury
  while staked, so the guild can count on them, and debited again on exit.
- `fund_guild_rewards(funder, alliance_id, amount)` splits a deposit across
  the guild's stakers pro rata; `claim_guild_rewards` pays a member's share.
- `unstake_from_guild` after the lock returns the principal and any pending
  rewards.

## Governance staking and slashing
The existing governance stake (`init_staking`, `stake_for_voting`,
`stake_with_tier`, `unstake_voting`, `get_voting_power`) is exposed on the
contract, and gains `slash_stake(admin, staker, bps)`: the staking admin can
remove a share of a stake for malicious voting. Voting power is derived from
the stake, so it drops with it. `get_total_slashed` reports the lifetime
total.

## TVL
- `get_staking_tvl(asset)`: units of one asset locked in resource stakes.
- `get_tvl_report()`: resource units locked across every asset, ships staked,
  and lifetime yield paid.
- `get_yield_reserve(asset)`: units available to pay yield.

## Security notes
- Staked assets never leave the contract: resource stakes debit the
  harvested balance into a stake record, ship stakes block transfers, LP
  stakes move units out of the AMM balance, guild stakes debit the balance.
- Yield is only ever paid from a funded reserve; nothing is minted.
- Every withdrawal path removes the position before crediting the balance.
- All amounts use checked or saturating arithmetic; overflow surfaces as
  `ArithmeticOverflow`.

## Tests
- `src/economics/apy_calculator.rs`: worked examples for every formula,
  including the reference impermanent-loss values.
- `tests/economy/staking_test.rs`: resource, NFT, LP, guild and governance
  flows through the contract client, including early-withdrawal penalties,
  the lock, the reserve, and the transfer freeze.
